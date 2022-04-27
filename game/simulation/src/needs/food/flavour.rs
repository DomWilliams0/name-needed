use std::str::FromStr;

use enumflags2::{bitflags, BitFlags};
use strum::EnumString;

use common::{Itertools, SmallVec};

#[bitflags]
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, EnumString)]
#[strum(serialize_all = "kebab-case")]
pub enum FoodFlavour {
    RawMeat,
    CookedMeat,
    RawPlant,
    CookedPlant,
    Fruit,
    Seeds,
}

/// Set of flavours that interests an entity
// TODO specify explicit dislikes too?
// TODO make this more compact - small integer type, or store a SoA in order of bitflag decl?
#[derive(Debug, Clone)]
pub struct FoodInterest(SmallVec<[(FoodFlavour, f32); 2]>);

/// Set of flavours presented by an item of food
#[derive(Debug, Clone)]
pub struct FoodFlavours(BitFlags<FoodFlavour>);

impl FoodInterest {
    pub fn eats(&self, food: &FoodFlavours) -> bool {
        // TODO use bitset AND
        food.0.iter().all(|f| self.eats_flavour(f))
    }

    fn eats_flavour(&self, flavour: FoodFlavour) -> bool {
        self.0.iter().any(|(f, _)| *f == flavour)
    }

    pub fn interests(&self) -> impl Iterator<Item = (FoodFlavour, f32)> + '_ {
        self.0.iter().copied()
    }
}

impl FromStr for FoodInterest {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut interests = vec![];

        let mut total = 0u32;
        for entry in s.split(',') {
            let (interest, preference) = entry.split_once('=').ok_or("missing =")?;
            let interest: FoodFlavour = interest.parse().map_err(|_| "unknown flavour")?;

            let preference: u32 = preference.parse().map_err(|_| "bad preference")?;
            total += preference;
            interests.push((interest, preference));
        }

        if interests.is_empty() {
            return Err("empty flavours");
        };
        if total == 0 {
            return Err("total interest is 0");
        };

        let total = total as f32;
        Ok(FoodInterest(
            interests
                .into_iter()
                .map(|(int, pref)| (int, pref as f32 / total))
                .collect(),
        ))
    }
}

impl FromStr for FoodFlavours {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split(',')
            .map(|s| s.parse::<FoodFlavour>().map_err(|_| "unknown flavour"))
            .try_collect::<_, _, _>()
            .map(FoodFlavours)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_check() {
        let wolf: FoodInterest = "raw-meat=10,cooked-meat=8,fruit=1"
            .parse()
            .expect("bad wolf input");
        let human: FoodInterest = "cooked-meat=50,fruit=40,cooked-plant=40"
            .parse()
            .expect("bad human input");

        let raw_meat: FoodFlavours = "raw-meat".parse().expect("bad meat");
        let apple: FoodFlavours = "fruit".parse().expect("bad fruit");
        let cooked_veg: FoodFlavours = "cooked-plant".parse().expect("bad veg");

        assert!(wolf.eats(&raw_meat));
        assert!(!human.eats(&raw_meat));

        assert!(wolf.eats(&apple));
        assert!(human.eats(&apple));

        assert!(!wolf.eats(&cooked_veg));
        assert!(human.eats(&cooked_veg));
    }
}
