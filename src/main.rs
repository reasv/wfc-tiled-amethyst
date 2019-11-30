extern crate image;
extern crate wfc;
extern crate direction;
extern crate rand;
extern crate coord_2d;
extern crate ron;

use std::num::NonZeroU32;
use std::collections::HashSet;
use std::collections::HashMap;

use std::fs::File;
use direction::{CardinalDirectionTable, CardinalDirections};
use wfc::{GlobalStats, ForbidPattern, ForbidInterface, Wrap, PatternId, PatternDescription, PatternTable, RunOwn, retry, wrap};
use wfc::ForbidNothing;
use rand::Rng;
pub use coord_2d::{Coord, Size};
pub use wrap::WrapXY;
use image::{DynamicImage, Rgba, RgbaImage};

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
struct AdjacencyRule {
    name: String,
    weight: u32,
    directions: Vec<Vec<u32>>,
    all_directions: Vec<u32>
}
#[derive(Debug, Deserialize, Clone)]
struct TileRules {
    rules: HashMap<u32, AdjacencyRule>
}
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


fn load_rules_file(path: &str) -> Result<TileRules, String>{
    let f = File::open(path).map_err(|e| format!("{}", e))?;
    match ron::de::from_reader(f){
        Ok(x) => return Ok(x),
        Err(e) => {
            println!("Failed to load config: {}", e);
            return Err(format!("{}", e));
        }
    }
}
fn build_stats(tr: &TileRules) -> Result<(GlobalStats, Vec<u32>), String> {
    // Keeps all tile ids in one consistent order
    let mut tids = Vec::new();
    // Maps tids to their position in the vector
    let mut tid_index: HashMap<u32, usize> = HashMap::new();
    // new rule map containing processed rules
    let mut rule_map: HashMap<u32, AdjacencyRule> = tr.rules.clone();
    for (tid, _r) in &tr.rules {
        tids.push(*tid);
        tid_index.insert(*tid, tids.len()-1);
        let mut rule = rule_map.get(tid).unwrap().clone();
        // If directions is empty, set it to 4 empty vectors
        if rule.directions.len() != 4 {
            rule.directions = vec![[].to_vec(); 4];
        }
        // Add all_directions ids to all 4 directions
        for d in 0..4 {
            rule.directions[d].extend(&rule.all_directions);
        }
        // "Mirror" adjacency rules: for example if we allow x to be west of this tile,
        // we need to allow this tile to be east of x.
        for d in 0..4 {
            for allowed_tileid in &rule.directions[d]{
                // gives us the opposite direction
                let mirror_direction = (d + 2) % 4;
                // get rule for other tile
                let mut tile_rule = match rule_map.get_mut(allowed_tileid) {
                    Some(r) => r,
                    None => {
                        return Err(format!("Tile referenced by other tile, but not in rules: {}", allowed_tileid))
                    }
                };
                if tile_rule.directions.len() != 4 {
                    tile_rule.directions = vec![[].to_vec(); 4];
                } else if tile_rule.directions[mirror_direction].contains(tid){
                    continue
                }
                // add our tileid to the allowed tiles for the other tile
                tile_rule.directions[mirror_direction].push(*tid);
            }
        }
        // finally insert the processed rule in the map
        rule_map.insert(*tid, rule);
    }
    // Create rules table for wfc library from rules_map
    let mut patterns: Vec<PatternDescription> = Vec::new();
    for i in 0..tids.len() {
        let rule = rule_map.get(&tids[i]).unwrap();
        let mut allowed_neighbours = CardinalDirectionTable::default();
        let mut direc = 0;
        println!("{:?}", rule.directions);
        for direction in CardinalDirections {
            // Convert tids to indices from `tids`
            let allowed_index = rule.directions[direc].iter().map(|tid| *tid_index.get(&tid).unwrap() as u32).collect::<Vec<u32>>();
            println!("{:?}", allowed_index);
            allowed_neighbours[direction] = allowed_index;
            direc += 1;
        }
        patterns.push(PatternDescription::new(NonZeroU32::new(rule.weight), allowed_neighbours));
    }

    let table = PatternTable::from_vec(patterns);
    let global_stats = GlobalStats::new(table);
    return Ok((global_stats, tids));
}

fn main() {
    let tr: TileRules = load_rules_file(&"adjacency.ron").expect("Failed to open");
    //println!("{:?}", tr);
    let (global_stats, tids) = build_stats(&tr).expect("Failed to Build stats");
    let output_size = Size::new(32, 32);
    let mut rng = rand::thread_rng();
    let mut border_tiles = HashSet::new();
    let grass = tids.iter().enumerate().find(|&r| *(r.1) == 0).unwrap().0;
    border_tiles.insert(grass as u32);
    let forbid = Forbid {
        pattern_ids: border_tiles,
    };
    let run = RunOwn::new_wrap_forbid(output_size, &global_stats, WrapXY, forbid, &mut rng);
    let result = run.collapse_retrying(retry::NumTimes(100), &mut rng);
    let wave = match result {
        Ok(w) => w,
        Err(s) => {
            println!("{:?}", s);
            return
        }
    };

    let colormap = map!{0 => Rgba { data: [46, 204, 113, 255] }, 
                        81 => Rgba { data: [230, 126, 34, 255] },
                        73 => Rgba { data: [142, 68, 173, 255] },
                        89 => Rgba { data: [142, 68, 173, 255] },
                        80 => Rgba { data: [142, 68, 173, 255] },
                        82 => Rgba { data: [142, 68, 173, 255] },
                        74 => Rgba { data: [142, 68, 173, 255] },
                        72 => Rgba { data: [142, 68, 173, 255] },
                        90 => Rgba { data: [142, 68, 173, 255] },
                        88 => Rgba { data: [142, 68, 173, 255] },
                        75 => Rgba { data: [41, 128, 185, 255] },
                        83 => Rgba { data: [41, 128, 185, 255] },
                        76 => Rgba { data: [41, 128, 185, 255] },
                        84 => Rgba { data: [41, 128, 185, 255] }
                    };
    let size = wave.grid().size();
    let mut rgba_image = RgbaImage::new(size.width(), size.height());
    wave.grid().enumerate().for_each(|(Coord { x, y }, cell)| {
        let colour = match cell.chosen_pattern_id() {
            Ok(pattern_id) => {
                *colormap.get(&tids[pattern_id as usize]).unwrap()
            },
            Err(_) => Rgba { data: [0, 0, 0, 0] },
        };
        rgba_image.put_pixel(x as u32, y as u32, colour);
    });
    let img = DynamicImage::ImageRgba8(rgba_image);
    img.save("output1.png").expect("Failed to save");
}
#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_ron() {
        let f = File::open(&"adjacency.ron").expect("Failed to open");
        let tr: TileRules = match ron::de::from_reader(f){
            Ok(x) => x,
            Err(e) => {
                println!("Failed to load config: {}", e);

                std::process::exit(1);
            }
        };
        println!("{:?}", tr);
        return ();
    }
}