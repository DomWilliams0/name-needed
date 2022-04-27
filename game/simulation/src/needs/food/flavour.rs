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
#[derive(Debug, Clone)]
pub struct FoodInterest {
    flavours: BitFlags<FoodFlavour>,
    /// Associated with set bits in `self.flavours` in order of iteration
    preferences: SmallVec<[f32; 2]>,
}

/// Set of flavours presented by an item of food
#[derive(Debug, Clone)]
pub struct FoodFlavours(BitFlags<FoodFlavour>);

impl FoodInterest {
    pub fn eats(&self, food: &FoodFlavours) -> bool {
        (food.0 & self.flavours) == food.0
    }

    pub fn interests(&self) -> impl Iterator<Item = (FoodFlavour, f32)> + '_ {
        self.flavours.iter().zip(self.preferences.iter().copied())
    }

    fn interest_for(&self, flavour: FoodFlavour) -> Option<f32> {
        self.flavours
            .iter()
            .position(|f| f == flavour)
            .map(|i| self.preferences[i])
    }
}

impl FromStr for FoodInterest {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut flavours = BitFlags::empty();
        let mut preferences = vec![];

        let mut total = 0u32;
        for entry in s.split(',') {
            let (interest, preference) = entry.split_once('=').ok_or("missing =")?;
            let flavour: FoodFlavour = interest.parse().map_err(|_| "unknown flavour")?;

            let preference: u32 = preference.parse().map_err(|_| "bad preference")?;
            preferences.push(preference);
            total += preference;
            flavours.insert(flavour);
        }

        if flavours.is_empty() {
            return Err("empty flavours");
        };
        if total == 0 {
            return Err("total interest is 0");
        };

        let total = total as f32;
        Ok(FoodInterest {
            flavours,
            preferences: preferences
                .into_iter()
                .map(|pref| pref as f32 / total)
                .collect(),
        })
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

    #[test]
    fn preferences() {
        let wolf: FoodInterest = "raw-meat=10,cooked-meat=8,fruit=1"
            .parse()
            .expect("bad wolf input");
        let calculate = |n| (n as f32) / (10 + 8 + 1) as f32;

        assert_eq!(
            wolf.interest_for(FoodFlavour::CookedMeat),
            Some(calculate(8))
        );
        assert_eq!(wolf.interest_for(FoodFlavour::RawMeat), Some(calculate(10)));
        assert_eq!(wolf.interest_for(FoodFlavour::Fruit), Some(calculate(1)));
        assert!(wolf.interest_for(FoodFlavour::RawPlant).is_none());
    }
}
