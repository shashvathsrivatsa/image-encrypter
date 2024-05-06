use std::env;
use std::time::Instant;

use image::{imageops, ImageResult, RgbImage};

use num_cpus;
use std::sync::Arc;
use rayon::prelude::*;

use std::cell::UnsafeCell;
use std::marker::Sync;

//-----------------------------------------------------------------------//

struct UnsafeImage {
    data: UnsafeCell<RgbImage>,
}

unsafe impl Sync for UnsafeImage {}

impl UnsafeImage {
    fn new(data: RgbImage) -> Self {
        UnsafeImage { data: UnsafeCell::new(data) }
    }

    fn save(&self, path: &str) -> Result<(), image::ImageError> {
        let img = unsafe { &*self.data.get() };
        img.save(path)
    }

}

//-----------------------------------------------------------------------//

fn open_image(input_path: &str) -> RgbImage {
    let mut img = image::open(input_path).unwrap().to_rgb8();

    let meta = rexiv2::Metadata::new_from_path(input_path).unwrap();
    let orientation = meta.get_tag_string("Exif.Image.Orientation").unwrap_or("1".to_string());
    if orientation == "6" {
        img = imageops::rotate90(&img);
    }

    img
}

fn main() {
    let start = Instant::now();

    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        panic!("Usage: {} <encrypt / decrypt> <input_image> <encrypted_image>", args[0]);
    }

    let function = &args[1];
    let input_path = &args[2];
    let encrypted_path = &args[3];

    match function.as_str() {
        "encrypt" => {
            match encrypt_image(input_path, encrypted_path) {
                Ok(()) => println!("\nSuccessfully encrypted the image."),
                Err(err) => eprintln!("Error: {}", err),
            }
        }

        "decrypt" => {
            match decrypt_image(input_path, encrypted_path) {
                Ok(()) => println!("\nSuccessfully decrypted the image."),
                Err(err) => eprintln!("Error: {}", err),
            }
        }

        _ => {
            panic!("Unknown operation. Please specify 'encrypt' or 'decrypt'.");
        }
    }

    let duration = start.elapsed();
    println!("total: {:?}", duration);
}

fn decrypt_image(input_path: &str, output_path: &str) -> ImageResult<()> {
    println!("\nopening...");
    let start = Instant::now();

    let img: RgbImage = open_image(input_path);
    let img = UnsafeImage::new(img);
    let shared_img = Arc::new(img);

    let duration = start.elapsed();
    println!("opened: {:?}", duration);

    const INCREMENT: i32 = -32;

    let modified_image: Arc<UnsafeImage> = shift_columns(shared_img, INCREMENT);
    let modified_image: Arc<UnsafeImage> = shift_rows(modified_image, INCREMENT);
    let modified_image: Arc<UnsafeImage> = shift_columns(modified_image, INCREMENT);
    let final_image: Arc<UnsafeImage> = shift_rows(modified_image, INCREMENT);

    println!("\nsaving...");
    let start = Instant::now();
    final_image.save(output_path)?;
    let duration = start.elapsed();
    println!("saved: {:?}", duration);
    Ok(())
}

fn encrypt_image(input_path: &str, output_path: &str) -> ImageResult<()> {
    println!("\nopening...");
    let start = Instant::now();

    let img: RgbImage = open_image(input_path);
    let img = UnsafeImage::new(img);
    let shared_img = Arc::new(img);

    let duration = start.elapsed();
    println!("opened: {:?}", duration);

    const INCREMENT: i32 = 32;

    let modified_image: Arc<UnsafeImage> = shift_rows(shared_img, INCREMENT);
    let modified_image: Arc<UnsafeImage> = shift_columns(modified_image, INCREMENT);
    let modified_image: Arc<UnsafeImage> = shift_rows(modified_image, INCREMENT);
    let final_image: Arc<UnsafeImage> = shift_columns(modified_image, INCREMENT);

    println!("\nsaving...");
    let start = Instant::now();
    final_image.save(output_path)?;
    let duration = start.elapsed();
    println!("saved: {:?}", duration);
    Ok(())
}

fn shift_rows(shared_img: Arc<UnsafeImage>, increment: i32) -> Arc<UnsafeImage> {
    println!("\nshifting rows...");
    let start = Instant::now();

    let (width, height) = unsafe {
        let img = &*shared_img.data.get();
        (img.width() as i32, img.height())
    };

    let num_threads = num_cpus::get() as u32;
    let chunk_size = height / num_threads;

    let final_image = Arc::new(UnsafeImage::new(RgbImage::new(width as u32, height)));

    (0..num_threads).into_par_iter().for_each(|thread_id| {
        let start_y = thread_id * chunk_size;
        let end_y = if thread_id == num_threads - 1 {
            height
        } else {
            (thread_id + 1) * chunk_size
        };

        for y in (start_y..end_y).rev() {
            let y_shift = (y as i32 * increment).rem_euclid(width);
            for x in 0..width {
                let shift = x + y_shift;
                let shift = if shift >= width { shift - width } else { shift };
                let pixel = unsafe { (*shared_img.data.get()).get_pixel(shift as u32, y) };
                unsafe { (*final_image.data.get()).put_pixel(x as u32, y, *pixel) };
            }
        }
    });

    let duration = start.elapsed();
    println!("{:?}", duration);

    final_image
}

fn shift_columns(shared_img: Arc<UnsafeImage>, increment: i32) -> Arc<UnsafeImage> {
    println!("\nshifting columns...");
    let start = Instant::now();

    let (width, height) = unsafe {
        let img = &*shared_img.data.get();
        (img.width(), img.height() as i32)
    };

    let num_threads = num_cpus::get() as u32;
    let chunk_size = width / num_threads;

    let final_image = Arc::new(UnsafeImage::new(RgbImage::new(width, height as u32)));

    (0..num_threads).into_par_iter().for_each(|thread_id| {
        let start_x = thread_id * chunk_size;
        let end_x = if thread_id == num_threads - 1 {
            width
        } else {
            (thread_id + 1) * chunk_size
        };

        for x in (start_x..end_x).rev() {
            let x_shift = (x as i32 * increment).rem_euclid(height);
            for y in 0..height {
                let shift = y + x_shift;
                let shift = if shift >= height { shift - height } else { shift };
                let pixel = unsafe { (*shared_img.data.get()).get_pixel(x, shift as u32) };
                unsafe { (*final_image.data.get()).put_pixel(x, y as u32, *pixel) };
            }
        }

    });

    let duration = start.elapsed();
    println!("{:?}", duration);

    final_image
}


