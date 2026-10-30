use std::boxed::Box;
use std::error::Error;

pub enum PixelProvider<'a> {
    RGB(usize, usize, &'a [u8]),
    RGB0(usize, usize, &'a [u8]),
    BGR0(usize, usize, &'a [u8]),
    BGR0S(usize, usize, usize, &'a [u8]),
    NONE,
}

impl<'a> PixelProvider<'a> {
    pub fn size(&self) -> (usize, usize) {
        match self {
            PixelProvider::RGB(w, h, _) => (*w, *h),
            PixelProvider::RGB0(w, h, _) => (*w, *h),
            PixelProvider::BGR0(w, h, _) => (*w, *h),
            PixelProvider::BGR0S(w, h, _, _) => (*w, *h),
            PixelProvider::NONE => (0, 0),
        }
    }
}

pub trait Recorder {
    fn capture(&mut self, timeout_ms: u64) -> Result<PixelProvider<'_>, Box<dyn Error>>;
}

pub trait BoxCloneCapturable {
    fn box_clone(&self) -> Box<dyn Capturable>;
}

impl<T> BoxCloneCapturable for T
where
    T: Clone + Capturable + 'static,
{
    fn box_clone(&self) -> Box<dyn Capturable> {
        Box::new(self.clone())
    }
}

pub trait Capturable: Send + BoxCloneCapturable {
    fn name(&self) -> String;
    fn geometry_relative(&self) -> Result<(f64, f64, f64, f64), Box<dyn Error>>;
    fn before_input(&mut self) -> Result<(), Box<dyn Error>>;
    fn recorder(&self, capture_cursor: bool) -> Result<Box<dyn Recorder>, Box<dyn Error>>;
}

impl Clone for Box<dyn Capturable> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}
