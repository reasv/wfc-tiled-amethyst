extern crate image;
extern crate wfc;
extern crate direction;
extern crate rand;
extern crate coord_2d;

use std::num::NonZeroU32;
use std::collections::HashSet;
use direction::{CardinalDirectionTable, CardinalDirections};
use wfc::{GlobalStats, ForbidPattern, ForbidInterface, ForbidNothing, Wrap, PatternId, PatternDescription, PatternTable, RunOwn, retry, wrap};
use rand::Rng;
pub use coord_2d::{Coord, Size};
pub use wrap::WrapXY;
use image::{DynamicImage, Rgba, RgbaImage};

macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);
struct Forbid {
    pattern_ids: HashSet<PatternId>,
}

impl ForbidPattern for Forbid {
    fn forbid<W: Wrap, R: Rng>(&mut self, fi: &mut ForbidInterface<W>, rng: &mut R) {
        let output_size = fi.wave_size();
        (0..(output_size.width() as i32))
            .map(|x| Coord::new(x, output_size.height() as i32 - 1 as i32))
            .chain(
                (0..(output_size.width() as i32)).map(|y| {
                    Coord::new(output_size.width() as i32 - 1 as i32, y)
                }),
            )
            .for_each(|coord| {
                self.pattern_ids.iter().for_each(|&pattern_id| {
                    fi.forbid_all_patterns_except(coord, pattern_id, rng)
                        .unwrap();
                });
            });
    }
}

fn main() {
    let rules = vec![vec![0,1], vec![1,2,0], vec![1,2]];
    let mut patterns = Vec::new();
    for allowed in rules {
        let weight = NonZeroU32::new(1);
        let mut allowed_neighbours = CardinalDirectionTable::default();
        for direction in CardinalDirections {
            allowed_neighbours[direction] = allowed.clone();
        }
        patterns.push(PatternDescription::new(weight, allowed_neighbours))
    }
    let table = PatternTable::from_vec(patterns);
    let global_stats = GlobalStats::new(table);
    let output_size = Size::new(32, 32);
    let mut rng = rand::thread_rng();
    let mut border_tiles = HashSet::new();
    border_tiles.insert(0);
    let forbid = Forbid {
        pattern_ids: border_tiles,
    };
    let run = RunOwn::new_wrap_forbid(output_size, &global_stats, WrapXY, forbid, &mut rng);
    let result = run.collapse_retrying(retry::NumTimes(10), &mut rng);
    let wave;
    match result {
        Ok(wave_res) => {
            wave = wave_res;
        },
        Err(s) => {
            println!("{:?}", s);
            return
        }
    }
    let colormap = map!{0 => Rgba { data: [1, 1, 1, 255] }, 1 => Rgba { data: [46, 204, 113, 255] }, 2 => Rgba { data: [142, 68, 173, 255] }};
    let size = wave.grid().size();
    let mut rgba_image = RgbaImage::new(size.width(), size.height());
    wave.grid().enumerate().for_each(|(Coord { x, y }, cell)| {
        let colour = match cell.chosen_pattern_id() {
            Ok(pattern_id) => {
                *colormap.get(&pattern_id).unwrap()
            },
            Err(_) => Rgba { data: [0, 0, 0, 0] },
        };
        rgba_image.put_pixel(x as u32, y as u32, colour);
    });
    let img = DynamicImage::ImageRgba8(rgba_image);
    img.save("output.png").expect("Failed to save");
}
