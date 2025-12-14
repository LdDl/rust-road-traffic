//! Color management system for object tracking visualization.
//!
//! This module provides functionality to generate distinct colors for different object classes
//! and create faded versions for lost/untracked objects. The color generation uses HSV color
//! space to ensure good visual distribution and readability.

use std::collections::HashMap;
use rand::Rng;
use opencv::{
    core::Scalar,
};

use crate::lib::constants::EPSILON;

/// A color management system that assigns distinct colors to object classes.
/// 
/// `ClassColors` generates a palette of visually distinct colors for different object classes
/// (e.g., car, truck, bus) and automatically creates faded versions of these colors to
/// represent lost or untracked objects. This provides clear visual distinction between
/// active tracking and objects that have temporarily lost tracking.
/// 
/// # Examples
/// 
/// ```
/// use crate::lib::draw::colors::ClassColors;
/// 
/// let class_names = vec!["car".to_string(), "truck".to_string(), "bus".to_string()];
/// let colors = ClassColors::new(&class_names);
/// 
/// // Get color for an active car
/// let car_color = colors.get_color("car");
/// 
/// // Get faded color for a lost car
/// let lost_car_color = colors.get_lost_color("car");
/// ```
pub struct ClassColors {
    /// Primary colors for each object class when actively tracked
    pub colors: HashMap<String, Scalar>,
    /// Faded colors for each object class when tracking is lost
    pub lost_colors: HashMap<String, Scalar>,
    /// Default color used when class is not found in the color map
    pub default_color: Scalar,
    /// Default faded color used when class is not found in the lost color map
    pub default_lost_color: Scalar,
}

impl ClassColors {
    /// Creates a new `ClassColors` instance with generated colors for the specified classes.
    /// 
    /// This method generates a palette of visually distinct colors using HSV color space
    /// for better distribution. Each class gets a primary color and a corresponding faded
    /// version for when tracking is lost.
    /// 
    /// # Arguments
    /// 
    /// * `class_names` - A slice of strings representing the object class names
    /// 
    /// # Returns
    /// 
    /// A new `ClassColors` instance with colors assigned to all provided classes
    /// 
    /// # Examples
    /// 
    /// ```
    /// let classes = vec!["car".to_string(), "truck".to_string()];
    /// let colors = ClassColors::new(&classes);
    /// ```
    pub fn new(class_names: &[String]) -> Self {
        let mut colors = HashMap::new();
        let mut lost_colors = HashMap::new();
        
        // Generate distinct colors using HSV color space for better distribution
        let color_pool = Self::generate_distinct_colors(class_names.len().max(50)); // At least 50 colors
        
        // Assign primary colors and create faded versions for lost objects
        for (i, class_name) in class_names.iter().enumerate() {
            let color_index = i % color_pool.len();
            let primary_color = color_pool[color_index];
            let faded_color = Self::create_faded_color(&primary_color);
            
            colors.insert(class_name.clone(), primary_color);
            lost_colors.insert(class_name.clone(), faded_color);
        }
        
        let default_color = Scalar::from((255.0, 255.0, 255.0)); // White
        let default_lost_color = Self::create_faded_color(&default_color);
        
        ClassColors {
            colors,
            lost_colors,
            default_color,
            default_lost_color,
        }
    }
    
    /// Creates a faded version of the given color by reducing saturation and brightness.
    /// 
    /// This method converts the color to HSV color space, reduces the saturation to 40%
    /// and brightness to 60%, then converts back to BGR format for OpenCV compatibility.
    /// This creates a natural "washed out" appearance suitable for representing lost objects.
    /// 
    /// # Arguments
    /// 
    /// * `color` - The original color in BGR format (OpenCV Scalar)
    /// 
    /// # Returns
    /// 
    /// A new Scalar representing the faded version of the input color
    /// 
    /// # Examples
    /// 
    /// ```
    /// use opencv::core::Scalar;
    /// 
    /// let bright_red = Scalar::from((0.0, 0.0, 255.0)); // BGR format
    /// let faded_red = ClassColors::create_faded_color(&bright_red);
    /// ```
    fn create_faded_color(color: &Scalar) -> Scalar {
        let b = color[0];
        let g = color[1];
        let r = color[2];
        
        // Method 1: Reduce saturation and brightness
        let (h, s, v) = Self::rgb_to_hsv(r as u8, g as u8, b as u8);
        let faded_s = s * 0.4; // Reduce saturation to 40%
        let faded_v = v * 0.6; // Reduce brightness to 60%
        let (faded_r, faded_g, faded_b) = Self::hsv_to_rgb(h, faded_s, faded_v);
        
        Scalar::from((faded_b as f64, faded_g as f64, faded_r as f64))
    }
    
    /// Converts RGB color values to HSV color space.
    /// 
    /// This conversion is used internally for color manipulation operations such as
    /// creating faded colors. The HSV color space allows for easier manipulation
    /// of color properties like saturation and brightness.
    /// 
    /// # Arguments
    /// 
    /// * `r` - Red component (0-255)
    /// * `g` - Green component (0-255)  
    /// * `b` - Blue component (0-255)
    /// 
    /// # Returns
    /// 
    /// A tuple containing (hue, saturation, value) where:
    /// - Hue is in degrees (0-360)
    /// - Saturation is normalized (0.0-1.0)
    /// - Value is normalized (0.0-1.0)
    fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
        let r = r as f32 / 255.0;
        let g = g as f32 / 255.0;
        let b = b as f32 / 255.0;
        
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        
        let h = if delta < EPSILON {
            0.0
        } else if (max - r).abs() < EPSILON {
            60.0 * (((g - b) / delta) % 6.0)
        } else if (max - g).abs() < EPSILON {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        
        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max;
        
        (h.max(0.0), s, v)
    }
    
    /// Generates a collection of visually distinct colors using HSV color space.
    /// 
    /// This method creates colors by distributing hues evenly across the color spectrum
    /// and varying saturation and brightness values to maximize visual distinction.
    /// The colors are then shuffled to prevent adjacent classes from having similar colors.
    /// 
    /// # Arguments
    /// 
    /// * `count` - The number of distinct colors to generate
    /// 
    /// # Returns
    /// 
    /// A vector of `Scalar` values representing colors in BGR format for OpenCV
    /// 
    /// # Algorithm
    /// 
    /// 1. Distributes hues evenly across 360° spectrum
    /// 2. Varies saturation (60%, 80%, 100%) and brightness (80%, 100%)
    /// 3. Adds small random hue offset to prevent systematic patterns
    /// 4. Shuffles final color list for better distribution
    fn generate_distinct_colors(count: usize) -> Vec<Scalar> {
        let mut colors = Vec::new();
        let mut rng = rand::rng();
        
        // Generate colors using HSV for better distribution
        for i in 0..count {
            // Distribute hues evenly across the spectrum
            let hue = (i as f32 * 360.0 / count as f32) % 360.0;
            
            // Vary saturation and value for more diversity
            let saturation = if i % 3 == 0 { 1.0 } else if i % 3 == 1 { 0.8 } else { 0.6 };
            let value = if i % 2 == 0 { 1.0 } else { 0.8 };
            
            // Add some randomness to prevent too systematic patterns
            let hue_offset = rng.random_range(-15.0..15.0);
            let final_hue = (hue + hue_offset).max(0.0).min(360.0);
            
            let (r, g, b) = Self::hsv_to_rgb(final_hue, saturation, value);
            colors.push(Scalar::from((b as f64, g as f64, r as f64))); // OpenCV uses BGR
        }
        
        // Shuffle the colors to avoid adjacent classes having similar colors
        use rand::seq::SliceRandom;
        colors.shuffle(&mut rng);
        
        colors
    }
    
    /// Converts HSV color values to RGB color space.
    /// 
    /// This is the inverse operation of `rgb_to_hsv` and is used to convert
    /// manipulated HSV values back to RGB for use with OpenCV.
    /// 
    /// # Arguments
    /// 
    /// * `h` - Hue in degrees (0-360)
    /// * `s` - Saturation normalized (0.0-1.0)
    /// * `v` - Value (brightness) normalized (0.0-1.0)
    /// 
    /// # Returns
    /// 
    /// A tuple containing (red, green, blue) components as u8 values (0-255)
    fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        
        let (r_prime, g_prime, b_prime) = match h as i32 {
            0..=59 => (c, x, 0.0),
            60..=119 => (x, c, 0.0),
            120..=179 => (0.0, c, x),
            180..=239 => (0.0, x, c),
            240..=299 => (x, 0.0, c),
            300..=359 => (c, 0.0, x),
            _ => (0.0, 0.0, 0.0),
        };
        
        let r = ((r_prime + m) * 255.0) as u8;
        let g = ((g_prime + m) * 255.0) as u8;
        let b = ((b_prime + m) * 255.0) as u8;
        
        (r, g, b)
    }
    
    /// Retrieves the primary color for a given class name.
    /// 
    /// Returns the assigned color for the specified class, or the default color
    /// if the class is not found in the color map.
    /// 
    /// # Arguments
    /// 
    /// * `class_name` - The name of the object class
    /// 
    /// # Returns
    /// 
    /// The color associated with the class, or the default color if not found
    /// 
    /// # Examples
    /// 
    /// ```
    /// let color = colors.get_color("car"); // Returns the car's assigned color
    /// let unknown_color = colors.get_color("unknown"); // Returns default color
    /// ```
    pub fn get_color(&self, class_name: &str) -> Scalar {
        self.colors.get(class_name).cloned().unwrap_or(self.default_color)
    }
    
    /// Retrieves the faded color for a given class name.
    /// 
    /// Returns the faded version of the assigned color for the specified class,
    /// used to represent lost or untracked objects. If the class is not found,
    /// returns the default faded color.
    /// 
    /// # Arguments
    /// 
    /// * `class_name` - The name of the object class
    /// 
    /// # Returns
    /// 
    /// The faded color associated with the class, or the default faded color if not found
    /// 
    /// # Examples
    /// 
    /// ```
    /// let lost_color = colors.get_lost_color("car"); // Returns the car's faded color
    /// let unknown_lost_color = colors.get_lost_color("unknown"); // Returns default faded color
    /// ```
    pub fn get_lost_color(&self, class_name: &str) -> Scalar {
        self.lost_colors.get(class_name).cloned().unwrap_or(self.default_lost_color)
    }
}