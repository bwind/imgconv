//! The following functions provide the tools to fit and crop images into given
//! dimensions, allowing zoom and focal points as a percentage of the original
//! image.
//!
//! The `fit` and `crop` functions work roughly identical apart from the fact that
//! `fit` never deletes data, while `crop` might. Both return a 2-item resize tuple
//! and a 4-item crop tuple.

#[derive(Debug)]
struct ValidationErr;

// x, y point expressed in pixels
struct Point {
    x: u32,
    y: u32,
}

// x, y point expressed in percentages
struct RelativePoint {
    x: f64,
    y: f64,
}

impl RelativePoint {
    pub fn build(x: f64, y: f64) -> Result<Self, ValidationErr> {
        for e in [x, y] {
            match e {
                e if e < 0. || e > 100. => return Err(ValidationErr),
                _ => (),
            }
        }
        Ok(Self { x, y })
    }
}

#[derive(Debug, PartialEq)]
struct Box {
    w: u32,
    h: u32,
}

impl Box {
    fn floats(&self) -> (f64, f64) {
        (self.w as f64, self.h as f64)
    }
}

// A Box that may have at most one missing value
struct OptionBox {
    w: Option<u32>,
    h: Option<u32>,
}

impl OptionBox {
    pub fn build(w: Option<u32>, h: Option<u32>) -> Result<Self, ValidationErr> {
        if w.is_some() || h.is_some() {
            return Ok(Self { w, h });
        }
        Err(ValidationErr)
    }
}

#[derive(Debug, PartialEq)]
struct CropBox {
    top: u32,
    left: u32,
    right: u32,
    bottom: u32,
}

/// Calculates the focal point as an absolute pixel position on a
/// one-dimensional axis (either width or height). It attempts to make the crop
/// area not exceed the image's edge – if however the crop area is larger than
/// the image itself, this function will always yield the center of the
/// axis.
///
/// # Examples
///
/// ```
/// use imgconv::true_focal_point;
/// let image_length = 1536;
/// let crop_length = 1280;
/// let focal_point = 50.;
/// let true_focal_point = true_focal_point(image_length, crop_length, focal_point);
/// assert_eq!(true_focal_point, 768);
/// ```
pub fn true_focal_point(image_length: u32, crop_length: u32, focal_point: f64) -> u32 {
    let space = get_space(image_length, crop_length);
    let rel = true_focal_point_rel(focal_point, space);
    (image_length as f64 * (rel / 100.)) as u32
}

fn true_focal_point_rel(focal_point: f64, space: f64) -> f64 {
    (focal_point.min(100.).max(0.) - 50.)
        .max((-space).min(0.))
        .min(space.max(0.))
        + 50.
}

fn get_space(image_length: u32, crop_length: u32) -> f64 {
    // Returns space between edge of image and bounding box as a percentage
    let image_length = image_length as f64;
    let crop_length = crop_length as f64;
    (image_length - crop_length) / (image_length / 100.) / 2.
}

fn crop_box(image_box: &Box, crop_box: &Box, focal_point: &RelativePoint) -> CropBox {
    // Given an image box (w, h), a crop box (w, h), and a focal point as a
    // percentage, returns the crop coordinates as pixels relative to the image's
    // dimensions.
    let true_focal_point = Point {
        x: true_focal_point(image_box.w, crop_box.w, focal_point.x) as u32,
        y: true_focal_point(image_box.h, crop_box.h, focal_point.y) as u32,
    };
    // TODO: too much casting going on, find other way to ensure positive (unsigned) values
    CropBox {
        top: (true_focal_point.x as i32 - (crop_box.w as f64 / 2.) as i32).max(0) as u32,
        left: (true_focal_point.y as i32 - (crop_box.h as f64 / 2.) as i32).max(0) as u32,
        bottom: (true_focal_point.x as i32 + (crop_box.w as f64 / 2.) as i32)
            .min(image_box.w as i32) as u32,
        right: (true_focal_point.y as i32 + (crop_box.h as f64 / 2.) as i32).min(image_box.h as i32)
            as u32,
    }
}

// If any of the sides in `resize_box` is None, calculate its length based on
// the aspect ratio of `image_box` and the length of the edge in `resize_box`.
fn add_missing_edge(image_box: &Box, resize_box: &OptionBox) -> Box {
    let (iw, ih) = image_box.floats();
    let (rw, rh) = (
        resize_box.w.unwrap_or_default() as f64,
        resize_box.h.unwrap_or_default() as f64,
    );
    let calc_edge =
        |i1: f64, i2: f64, r1: f64, r2: Option<u32>| r2.unwrap_or(((i1 / i2) * r1) as u32);
    let w = calc_edge(iw, ih, rh, resize_box.w);
    let h = calc_edge(ih, iw, rw, resize_box.h);
    Box { w, h }
}

// Resizes (fits) an image into a resize box, then zooms the result.
fn resize_and_zoom(image_box: &Box, resize_box: &Box, zoom: &Option<f64>) -> Box {
    let zoom = zoom.unwrap_or(1.);
    let (iw, ih) = image_box.floats();
    let (rw, rh) = resize_box.floats();
    let resize_factor = (iw / rw).max(ih / rh);
    let w = (iw / resize_factor * zoom) as u32;
    let h = (ih / resize_factor * zoom) as u32;
    Box { w, h }
}

// Crops an image into a crop box, then zooms the result.
fn crop_and_zoom(image_box: &Box, crop_box: &Box, zoom: &Option<f64>) -> Box {
    let zoom = zoom.unwrap_or(1.);
    let (iw, ih) = image_box.floats();
    let (cw, ch) = crop_box.floats();
    let resize_factor = (iw / cw).min(ih / ch);
    let w = (iw / resize_factor * zoom) as u32;
    let h = (ih / resize_factor * zoom) as u32;
    Box { w, h }
}

// Fits, then crops.
fn fit(
    image_box: &Box,
    resize_box: &OptionBox,
    focal_point: &RelativePoint,
    zoom: &Option<f64>,
) -> (Box, CropBox) {
    let resize_box = add_missing_edge(&image_box, &resize_box);
    let resized_and_zoomed = resize_and_zoom(&image_box, &resize_box, &zoom);
    let cropped = crop_box(&resized_and_zoomed, &resize_box, &focal_point);
    (resized_and_zoomed, cropped)
}

// Crop resizes, then crops.
fn crop(
    image_box: &Box,
    resize_box: &Box,
    focal_point: &RelativePoint,
    zoom: &Option<f64>,
) -> (Box, CropBox) {
    let resized_and_zoomed = crop_and_zoom(&image_box, &resize_box, &zoom);
    let cropped = crop_box(&resized_and_zoomed, &resize_box, &focal_point);
    (resized_and_zoomed, cropped)
}
