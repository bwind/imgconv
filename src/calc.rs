//! The following functions provide the tools to fit and crop images into given
//! dimensions, allowing zoom and focal points as a percentage of the original
//! image.
//!
//! The `fit` and `crop` functions work roughly identical apart from the fact that
//! `fit` never deletes data, while `crop` might. Both return a 2-item resize tuple
//! and a 4-item crop tuple.

#[derive(Debug)]
pub struct ValidationErr;

// x, y point expressed in pixels
pub struct Point {
    x: u32,
    y: u32,
}

// x, y point expressed in percentages
#[derive(Debug)]
pub struct RelativePoint {
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

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }
}

#[derive(Debug, PartialEq)]
pub struct Box {
    pub w: u32,
    pub h: u32,
}

impl Box {
    pub fn floats(&self) -> (f64, f64) {
        (self.w as f64, self.h as f64)
    }
}

// A Box that may have at most one missing value
pub struct OptionBox {
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

    pub fn w(&self) -> Option<u32> {
        self.w
    }

    pub fn h(&self) -> Option<u32> {
        self.h
    }
}

#[derive(Debug, PartialEq)]
pub struct CropBox {
    pub top: u32,
    pub left: u32,
    pub right: u32,
    pub bottom: u32,
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
    let calc_edge = |i1: f64, i2: f64, r1: Option<u32>, r2: Option<u32>| {
        r2.unwrap_or_else(|| ((i1 / i2) * (r1.unwrap() as f64)) as u32)
    };
    let w = calc_edge(iw, ih, resize_box.h, resize_box.w);
    let h = calc_edge(ih, iw, resize_box.w, resize_box.h);
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
pub fn fit(
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
pub fn crop(
    image_box: &Box,
    resize_box: &Box,
    focal_point: &RelativePoint,
    zoom: &Option<f64>,
) -> (Box, CropBox) {
    let resized_and_zoomed = crop_and_zoom(&image_box, &resize_box, &zoom);
    let cropped = crop_box(&resized_and_zoomed, &resize_box, &focal_point);
    (resized_and_zoomed, cropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_box_is_ok() {
        assert!(OptionBox::build(Some(100), Some(200)).is_ok());
        assert!(OptionBox::build(None, Some(200)).is_ok());
        assert!(OptionBox::build(Some(100), None).is_ok());
    }

    #[test]
    fn test_option_box_is_err() {
        assert!(OptionBox::build(None, None).is_err());
    }

    #[test]
    fn test_relative_point_validates() {
        assert!(RelativePoint::build(0., 100.).is_ok());
    }

    #[test]
    fn test_relative_point_does_not_validate() {
        assert!(RelativePoint::build(0., 100.1).is_err());
        assert!(RelativePoint::build(-0.1, 100.).is_err());
    }

    #[test]
    fn test_true_focal_point_rel() {
        assert_eq!(
            true_focal_point_rel(0., 8.333333333333334),
            41.6666666666666664,
        );
    }

    #[test]
    fn test_true_focal_point_out_of_bounds_upper() {
        assert_eq!(true_focal_point(1536, 1280, 100.0), 896);
    }

    #[test]
    fn test_true_focal_point_out_of_bounds_lower() {
        assert_eq!(true_focal_point(1536, 1280, 0.), 640);
    }

    #[test]
    fn test_true_focal_point_middle() {
        assert_eq!(true_focal_point(1536, 1280, 50.), 768);
    }

    #[test]
    fn test_true_focal_point_crop_is_larger_than_image_centers_focal_point() {
        assert_eq!(true_focal_point(1024, 1280, 100.), 512);
    }

    #[test]
    fn test_get_space() {
        assert_eq!(get_space(1536, 1280), 8.333333333333334);
    }

    #[test]
    fn test_crop_box_out_of_bounds_upper() {
        assert_eq!(
            crop_box(
                &Box { w: 1536, h: 1152 },
                &Box { w: 1280, h: 720 },
                &RelativePoint { x: 100., y: 100. }
            ),
            CropBox {
                top: 256,
                left: 432,
                bottom: 1536,
                right: 1152,
            }
        );
    }

    #[test]
    fn test_crop_box_out_of_bounds_lower() {
        assert_eq!(
            crop_box(
                &Box { w: 1536, h: 1152 },
                &Box { w: 1280, h: 720 },
                &RelativePoint { x: 0., y: 0. }
            ),
            CropBox {
                top: 0,
                left: 0,
                bottom: 1280,
                right: 720,
            }
        );
    }

    #[test]
    fn test_crop_box_middle() {
        assert_eq!(
            crop_box(
                &Box { w: 1536, h: 1152 },
                &Box { w: 1280, h: 720 },
                &RelativePoint { x: 50., y: 50. }
            ),
            CropBox {
                top: 128,
                left: 216,
                bottom: 1408,
                right: 936,
            }
        );
    }

    #[test]
    fn test_add_missing_edge_height() {
        assert_eq!(
            add_missing_edge(
                &Box { w: 100, h: 50 },
                &OptionBox {
                    w: Some(50),
                    h: None
                }
            ),
            Box { w: 50, h: 25 }
        );
    }

    #[test]
    fn test_add_missing_edge_width() {
        assert_eq!(
            add_missing_edge(
                &Box { w: 100, h: 50 },
                &OptionBox {
                    w: None,
                    h: Some(250)
                }
            ),
            Box { w: 500, h: 250 }
        );
    }

    #[test]
    fn test_resize_and_zoom() {
        assert_eq!(
            resize_and_zoom(
                &Box { w: 1920, h: 1440 },
                &Box { w: 1280, h: 720 },
                &Some(1.2)
            ),
            Box { w: 1152, h: 864 }
        );
    }

    #[test]
    fn test_crop_and_zoom() {
        assert_eq!(
            crop_and_zoom(
                &Box { w: 1920, h: 1440 },
                &Box { w: 1280, h: 720 },
                &Some(1.2)
            ),
            Box { w: 1536, h: 1152 }
        );
    }

    #[test]
    fn test_fit_same_aspect_ratio() {
        assert_eq!(
            fit(
                &Box { w: 1920, h: 1440 },
                &OptionBox {
                    w: Some(640),
                    h: Some(480)
                },
                &RelativePoint { x: 50., y: 50. },
                &None,
            ),
            (
                Box { w: 640, h: 480 },
                CropBox {
                    top: 0,
                    left: 0,
                    bottom: 640,
                    right: 480
                }
            )
        );
    }

    #[test]
    fn test_fit_yields_narrower_image() {
        assert_eq!(
            fit(
                &Box { w: 1920, h: 1440 },
                &OptionBox {
                    w: Some(1280),
                    h: Some(720)
                },
                &RelativePoint { x: 50., y: 50. },
                &None
            ),
            (
                Box { w: 960, h: 720 },
                CropBox {
                    top: 0,
                    left: 0,
                    bottom: 960,
                    right: 720
                },
            )
        );
    }

    #[test]
    fn test_fit_yields_shorter_image() {
        assert_eq!(
            fit(
                &Box { w: 1920, h: 1440 },
                &OptionBox {
                    w: Some(1280),
                    h: Some(1280)
                },
                &RelativePoint { x: 50., y: 50. },
                &None
            ),
            (
                Box { w: 1280, h: 960 },
                CropBox {
                    top: 0,
                    left: 0,
                    bottom: 1280,
                    right: 960
                }
            )
        );
    }

    #[test]
    fn test_fit_with_zoom_removes_top_and_bottom() {
        assert_eq!(
            fit(
                &Box { w: 1920, h: 1440 },
                &OptionBox {
                    w: Some(1280),
                    h: Some(720)
                },
                &RelativePoint { x: 50., y: 50. },
                &Some(1.2)
            ),
            (
                Box { w: 1152, h: 864 },
                CropBox {
                    top: 0,
                    left: 72,
                    bottom: 1152,
                    right: 792
                }
            )
        );
    }

    #[test]
    fn test_fit_without_width_calculates_width() {
        assert_eq!(
            fit(
                &Box { w: 1920, h: 1440 },
                &OptionBox {
                    w: Some(1280),
                    h: None
                },
                &RelativePoint { x: 50., y: 50. },
                &None
            ),
            (
                Box { w: 1280, h: 960 },
                CropBox {
                    top: 0,
                    left: 0,
                    bottom: 1280,
                    right: 960
                }
            )
        );
    }

    #[test]
    fn test_crop_same_aspect_ratio() {
        assert_eq!(
            crop(
                &Box { w: 1920, h: 1440 },
                &Box { w: 640, h: 480 },
                &RelativePoint { x: 50., y: 50. },
                &None
            ),
            (
                Box { w: 640, h: 480 },
                CropBox {
                    top: 0,
                    left: 0,
                    bottom: 640,
                    right: 480
                }
            )
        );
    }

    #[test]
    fn test_crop_removes_top_and_bottom() {
        assert_eq!(
            crop(
                &Box { w: 1920, h: 1440 },
                &Box { w: 1280, h: 720 },
                &RelativePoint { x: 50., y: 50. },
                &None
            ),
            (
                Box { w: 1280, h: 960 },
                CropBox {
                    top: 0,
                    left: 120,
                    bottom: 1280,
                    right: 840
                }
            )
        );
    }

    #[test]
    fn test_crop_removes_sides() {
        assert_eq!(
            crop(
                &Box { w: 1920, h: 1440 },
                &Box { w: 960, h: 960 },
                &RelativePoint { x: 50., y: 50. },
                &None,
            ),
            (
                Box { w: 1280, h: 960 },
                CropBox {
                    top: 160,
                    left: 0,
                    bottom: 1120,
                    right: 960,
                },
            ),
        );
    }

    #[test]
    fn test_crop_with_zoom_removes_sides_and_top_and_bottom() {
        assert_eq!(
            crop(
                &Box { w: 1920, h: 1440 },
                &Box { w: 1280, h: 720 },
                &RelativePoint { x: 50., y: 50. },
                &Some(1.2)
            ),
            (
                Box { w: 1536, h: 1152 },
                CropBox {
                    top: 128,
                    left: 216,
                    bottom: 1408,
                    right: 936
                }
            )
        );
    }
}
