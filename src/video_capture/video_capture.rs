use opencv::{prelude::*, videoio::VideoCapture, videoio::CAP_ANY};

pub fn get_video_capture(video_src: &str, typ: String) -> VideoCapture {
    if typ == "rtsp" {
        let video_capture = match VideoCapture::from_file(video_src, CAP_ANY) {
            Ok(result) => result,
            Err(err) => {
                panic!("Can't init '{}' due the error: {:?}", video_src, err);
            }
        };
        return video_capture;
    }
    let device_id = match video_src.parse::<i32>() {
        Ok(result) => result,
        Err(err) => {
            panic!(
                "Can't parse '{}' as device_id (i32) due the error: {:?}",
                video_src, err
            );
        }
    };
    let mut video_capture = match VideoCapture::new(device_id, CAP_ANY) {
        Ok(result) => result,
        Err(err) => {
            panic!("Can't init '{}' due the error: {:?}", video_src, err);
        }
    };
    return video_capture;
}
