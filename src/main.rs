mod calc;

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_web_validator::Query;
use image::io::Reader as ImageReader;
use image::{imageops, GenericImageView};
use serde::Deserialize;
use std::io::Cursor;
use std::str;
use std::str::FromStr;
use validator::{Validate, ValidationError};

const MEDIA_TYPES: [&str; 3] = ["jpeg", "png", "webp"];

#[derive(Debug, PartialEq)]
pub enum MediaType {
    JPEG,
    WEBP,
    PNG,
}

impl MediaType {
    const DEFAULT: Self = Self::WEBP;
}

impl FromStr for MediaType {
    type Err = ();

    fn from_str(input: &str) -> Result<MediaType, Self::Err> {
        match input {
            "jpeg" => Ok(Self::JPEG),
            "png" => Ok(Self::PNG),
            "webp" => Ok(Self::WEBP),
            _ => Err(()),
        }
    }
}

pub const DEFAULT_MEDIA_TYPE: MediaType = MediaType::JPEG;

pub const DEFAULT_QUALITY: [(MediaType, u8); 2] = [(MediaType::JPEG, 70), (MediaType::WEBP, 60)];

#[derive(Deserialize, Debug)]
struct PathInfo {
    signature: String,
    organization_id: String,
    media_id: String,
}

#[derive(Deserialize, Validate, Debug)]
#[validate(schema(function = "validate_query_info", skip_on_field_errors = false))]
struct QueryInfo {
    #[validate(custom = "validate_resize")]
    resize: Option<String>,
    w: Option<u32>,
    h: Option<u32>,
    #[validate(range(min = 0.5, max = 2.))]
    zoom: Option<f64>,
    #[validate(custom = "validate_media_type")]
    media_type: Option<String>,
    #[validate(range(min = 0, max = 100))]
    quality: Option<u8>,
    #[validate(range(min = 0., max = 100.))]
    fx: Option<f64>,
    #[validate(range(min = 0., max = 100.))]
    fy: Option<f64>,
    // blur: Option<f64>,
    // grayscale: Option<bool>,
    // bgcolor: Option<String>,
    // debug: Option<bool>,
}

impl QueryInfo {
    const DEFAULT_RESIZE: &str = "fit";
    const DEFAULT_FX: f64 = 50.;
    const DEFAULT_FY: f64 = 50.;

    pub fn get_default_quality_for_media_type(media_type: &MediaType) -> Result<u8, &'static str> {
        for (media_type_2, default_quality) in DEFAULT_QUALITY.into_iter() {
            if &media_type_2 == media_type {
                return Ok(default_quality);
            }
        }
        Err("Media type does not support quality")
    }
}

fn validate_query_info(query_info: &QueryInfo) -> Result<(), ValidationError> {
    if query_info.w == None && query_info.h == None {
        return Err(ValidationError::new(
            "At least one of `w`, `h` must be provided",
        ));
    }
    if query_info.resize == Some("crop".to_owned())
        && (query_info.h == None || query_info.w == None)
    {
        return Err(ValidationError::new(
            "For resize `crop` both `w` and `h` must be provided",
        ));
    }
    let media_type = match &query_info.media_type {
        Some(m) => MediaType::from_str(m).unwrap(),
        None => MediaType::DEFAULT,
    };
    if QueryInfo::get_default_quality_for_media_type(&media_type).is_err()
        && query_info.quality.is_some()
    {
        return Err(ValidationError::new("Media type does not support quality"));
    }

    Ok(())
}

fn validate_resize(resize: &str) -> Result<(), ValidationError> {
    if !["fit", "crop"].contains(&resize) {
        return Err(ValidationError::new(
            "resize must be either `fit` or `crop`",
        ));
    }
    Ok(())
}

fn validate_media_type(media_type: &str) -> Result<(), ValidationError> {
    if !MEDIA_TYPES.contains(&media_type) {
        return Err(ValidationError::new(
            "Media type must be `jpeg`, `png`, or `webp`",
        ));
    }
    Ok(())
}

#[get("/{signature}/{organization_id}/{media_id}")]
async fn transcode(query: Query<QueryInfo>, path: web::Path<PathInfo>) -> impl Responder {
    let resize = query
        .resize
        .to_owned()
        .unwrap_or(QueryInfo::DEFAULT_RESIZE.to_owned());
    // let media_type = match &query.media_type {
    //     Some(m) => MediaType::from_str(m).unwrap(),
    //     None => MediaType::DEFAULT,
    // };
    // let default_quality = QueryInfo::get_default_quality_for_media_type(&media_type);
    // let quality = if default_quality.is_err() {
    //     None
    // } else {
    //     Some(query.quality.unwrap_or_else(|| default_quality.unwrap()))
    // };
    let fx = query.fx.unwrap_or(QueryInfo::DEFAULT_FX);
    let fy = query.fy.unwrap_or(QueryInfo::DEFAULT_FY);

    let mut source = ImageReader::open("data/deventer.jpg")
        .unwrap()
        .decode()
        .unwrap();
    let dimensions = source.dimensions();

    let image_box = calc::Box {
        w: dimensions.0,
        h: dimensions.1,
    };
    let focal_point = calc::RelativePoint::build(fx, fy).unwrap();

    let result = match resize.as_str() {
        "fit" => calc::fit(
            &image_box,
            &calc::OptionBox::build(query.w, query.h).unwrap(),
            &focal_point,
            &query.zoom,
        ),
        _ => calc::crop(
            &image_box,
            &calc::Box {
                w: query.w.unwrap(),
                h: query.h.unwrap(),
            },
            &focal_point,
            &query.zoom,
        ),
    };

    let mut resized = imageops::resize(
        &mut source,
        result.0.w,
        result.0.h,
        imageops::FilterType::CatmullRom,
    );
    let cropped = imageops::crop(
        &mut resized,
        result.1.top,
        result.1.left,
        result.1.bottom - result.1.top,
        result.1.right - result.1.left,
    )
    .to_image();

    let mut bytes: Vec<u8> = Vec::new();
    cropped
        .write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Png)
        .unwrap();

    HttpResponse::Ok()
        .append_header(("Content-Type", "image/png"))
        .body(bytes)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(transcode))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
