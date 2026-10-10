//! Game data: items, transforms, recipes, levels, scoring.
//! Step-based design: Step 1 = Burger Bistro, Step 2 = Stew House, Step 3 = Veggie Cafe.

use std::collections::HashMap;

/// Every carryable / processable thing in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    // raw
    Bun,
    BurgerRaw,
    Carrot,
    Cheese,
    Ham,
    Lettuce,
    Onion,
    Potato,
    Steak,
    Tomato,
    VegPattyRaw,
    // chopped
    CarrotChopped,
    CheeseSlice,
    LettuceSlice,
    LettuceChopped,
    OnionRings,
    OnionChopped,
    PotatoChopped,
    PotatoMashed,
    SteakPieces,
    TomatoSlice,
    CarrotPieces,
    // cooked
    BurgerCooked,
    BurgerBurned,
    HamCooked,
    HamBurned,
    SteakCooked,
    VegPattyCooked,
    VegPattyBurned,
    // vessels / dishes
    PlateClean,
    PlateDirty,
    BowlClean,
    StewCooked,
    // step 6: noodles + charred failures
    NoodleBrick,
    NoodlesCooked,
    Charred,
}

impl Item {
    pub fn display_name(&self) -> &'static str {
        match self {
            Item::Bun => "Bun",
            Item::BurgerRaw => "Raw Patty",
            Item::Carrot => "Carrot",
            Item::Cheese => "Cheese",
            Item::Ham => "Ham",
            Item::Lettuce => "Lettuce",
            Item::Onion => "Onion",
            Item::Potato => "Potato",
            Item::Steak => "Steak",
            Item::Tomato => "Tomato",
            Item::VegPattyRaw => "Veg Patty (raw)",
            Item::CarrotChopped => "Chopped Carrot",
            Item::CheeseSlice => "Cheese Slice",
            Item::LettuceSlice => "Lettuce Slice",
            Item::LettuceChopped => "Chopped Lettuce",
            Item::OnionRings => "Onion Rings",
            Item::OnionChopped => "Chopped Onion",
            Item::PotatoChopped => "Chopped Potato",
            Item::PotatoMashed => "Mashed Potato",
            Item::SteakPieces => "Steak Pieces",
            Item::TomatoSlice => "Tomato Slice",
            Item::CarrotPieces => "Carrot Pieces",
            Item::BurgerCooked => "Cooked Patty",
            Item::BurgerBurned => "Burned Patty!",
            Item::HamCooked => "Cooked Ham",
            Item::HamBurned => "Burned Ham!",
            Item::SteakCooked => "Cooked Steak",
            Item::VegPattyCooked => "Cooked Veg Patty",
            Item::VegPattyBurned => "Burned Veg Patty!",
            Item::PlateClean => "Clean Plate",
            Item::PlateDirty => "Dirty Plate",
            Item::BowlClean => "Clean Bowl",
            Item::StewCooked => "Stew Pot",
            Item::NoodleBrick => "Instant Noodle Brick",
            Item::NoodlesCooked => "Cooked Noodles",
            Item::Charred => "Charred Mess!",
        }
    }

    /// glTF model file for this item (under assets/models/).
    pub fn model_file(&self) -> &'static str {
        match self {
            Item::Bun => "food_ingredient_bun.gltf",
            Item::BurgerRaw => "food_ingredient_burger_uncooked.gltf",
            Item::Carrot => "food_ingredient_carrot.gltf",
            Item::Cheese => "food_ingredient_cheese.gltf",
            Item::Ham => "food_ingredient_ham.gltf",
            Item::Lettuce => "food_ingredient_lettuce.gltf",
            Item::Onion => "food_ingredient_onion.gltf",
            Item::Potato => "food_ingredient_potato.gltf",
            Item::Steak => "food_ingredient_steak.gltf",
            Item::Tomato => "food_ingredient_tomato.gltf",
            Item::VegPattyRaw => "food_ingredient_vegetableburger_uncooked.gltf",
            Item::CarrotChopped => "food_ingredient_carrot_chopped.gltf",
            Item::CheeseSlice => "food_ingredient_cheese_slice.gltf",
            Item::LettuceSlice => "food_ingredient_lettuce_slice.gltf",
            Item::LettuceChopped => "food_ingredient_lettuce_chopped.gltf",
            Item::OnionRings => "food_ingredient_onion_rings.gltf",
            Item::OnionChopped => "food_ingredient_onion_chopped.gltf",
            Item::PotatoChopped => "food_ingredient_potato_chopped.gltf",
            Item::PotatoMashed => "food_ingredient_potato_mashed.gltf",
            Item::SteakPieces => "food_ingredient_steak_pieces.gltf",
            Item::TomatoSlice => "food_ingredient_tomato_slice.gltf",
            Item::CarrotPieces => "food_ingredient_carrot_pieces.gltf",
            Item::BurgerCooked => "food_ingredient_burger_cooked.gltf",
            Item::BurgerBurned => "food_ingredient_burger_trash.gltf",
            Item::HamCooked => "food_ingredient_ham_cooked.gltf",
            Item::HamBurned => "food_ingredient_ham_trash.gltf",
            Item::SteakCooked => "food_ingredient_steak_pieces.gltf",
            Item::VegPattyCooked => "food_ingredient_vegetableburger_cooked.gltf",
            Item::VegPattyBurned => "food_ingredient_burger_trash.gltf",
            Item::PlateClean => "plate.gltf",
            Item::PlateDirty => "plate_dirty.gltf",
            Item::BowlClean => "bowl_small.gltf",
            Item::StewCooked => "pot_A_stew.gltf",
            Item::NoodleBrick => "food_ingredient_carrot_pieces.gltf",
            Item::NoodlesCooked => "food_ingredient_potato_mashed.gltf",
            Item::Charred => "food_ingredient_burger_trash.gltf",
        }
    }

    pub fn is_burned(&self) -> bool {
        matches!(
            self,
            Item::BurgerBurned | Item::HamBurned | Item::VegPattyBurned | Item::Charred
        )
    }

    pub fn is_food(&self) -> bool {
        !matches!(
            self,
            Item::PlateClean | Item::PlateDirty | Item::BowlClean
        )
    }
}

/// What a station does to an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessKind {
    Chop,
    Cook,
    Bake,
    Wash,
}

/// (input -> output, station kind name, seconds, burn_after)
pub fn chop_result(input: Item) -> Option<(Item, f32)> {
    match input {
        Item::Tomato => Some((Item::TomatoSlice, 2.0)),
        Item::Lettuce => Some((Item::LettuceSlice, 2.0)),
        Item::LettuceSlice => Some((Item::LettuceChopped, 2.0)),
        Item::Onion => Some((Item::OnionRings, 2.0)),
        Item::OnionRings => Some((Item::OnionChopped, 1.5)),
        Item::Carrot => Some((Item::CarrotPieces, 2.0)),
        Item::CarrotPieces => Some((Item::CarrotChopped, 2.0)),
        Item::Cheese => Some((Item::CheeseSlice, 2.0)),
        Item::Potato => Some((Item::PotatoChopped, 2.5)),
        Item::Steak => Some((Item::SteakPieces, 3.0)),
        _ => None,
    }
}

pub fn cook_result(input: Item) -> Option<(Item, f32, f32)> {
    // (output, cook_time, burn_after_extra)
    match input {
        Item::BurgerRaw => Some((Item::BurgerCooked, 4.0, 5.0)),
        Item::VegPattyRaw => Some((Item::VegPattyCooked, 4.0, 5.0)),
        Item::Ham => Some((Item::HamCooked, 3.0, 5.0)),
        Item::SteakPieces => Some((Item::SteakCooked, 5.0, 5.0)),
        Item::NoodleBrick => Some((Item::NoodlesCooked, 3.0, 6.0)),
        _ => None,
    }
}

pub fn burned_version(cooked: Item) -> Item {
    match cooked {
        Item::BurgerCooked | Item::BurgerRaw => Item::BurgerBurned,
        Item::HamCooked | Item::Ham => Item::HamBurned,
        Item::VegPattyCooked | Item::VegPattyRaw => Item::VegPattyBurned,
        _ => Item::Charred,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecipeId {
    ClassicBurger,
    Cheeseburger,
    DeluxeBurger,
    HeartyStew,
    SundayDinner,
    VeggieBurger,
    VeggieDeluxe,
    NoodleBowl,
}

impl RecipeId {
    pub fn name(&self) -> &'static str {
        match self {
            RecipeId::ClassicBurger => "Classic Burger",
            RecipeId::Cheeseburger => "Cheeseburger",
            RecipeId::DeluxeBurger => "Deluxe Burger",
            RecipeId::HeartyStew => "Hearty Stew",
            RecipeId::SundayDinner => "Sunday Dinner",
            RecipeId::VeggieBurger => "Veggie Burger",
            RecipeId::VeggieDeluxe => "Veggie Deluxe",
            RecipeId::NoodleBowl => "Noodle Bowl",
        }
    }

    pub fn components(&self) -> &'static [Item] {
        match self {
            RecipeId::ClassicBurger => &[
                Item::Bun,
                Item::BurgerCooked,
                Item::LettuceSlice,
                Item::TomatoSlice,
            ],
            RecipeId::Cheeseburger => &[
                Item::Bun,
                Item::BurgerCooked,
                Item::CheeseSlice,
                Item::LettuceSlice,
            ],
            RecipeId::DeluxeBurger => &[
                Item::Bun,
                Item::BurgerCooked,
                Item::CheeseSlice,
                Item::TomatoSlice,
                Item::OnionRings,
                Item::LettuceSlice,
            ],
            RecipeId::HeartyStew => &[
                Item::CarrotChopped,
                Item::PotatoChopped,
                Item::OnionChopped,
            ],
            RecipeId::SundayDinner => &[
                Item::HamCooked,
                Item::PotatoMashed,
                Item::CarrotChopped,
            ],
            RecipeId::VeggieBurger => &[
                Item::Bun,
                Item::VegPattyCooked,
                Item::LettuceSlice,
                Item::TomatoSlice,
            ],
            RecipeId::VeggieDeluxe => &[
                Item::Bun,
                Item::VegPattyCooked,
                Item::CheeseSlice,
                Item::OnionChopped,
                Item::CarrotChopped,
                Item::LettuceChopped,
            ],
            RecipeId::NoodleBowl => &[
                Item::NoodlesCooked,
                Item::OnionChopped,
                Item::CarrotChopped,
            ],
        }
    }

    pub fn base_points(&self) -> u32 {
        match self {
            RecipeId::ClassicBurger => 100,
            RecipeId::Cheeseburger => 130,
            RecipeId::DeluxeBurger => 180,
            RecipeId::HeartyStew => 150,
            RecipeId::SundayDinner => 170,
            RecipeId::VeggieBurger => 120,
            RecipeId::VeggieDeluxe => 200,
            RecipeId::NoodleBowl => 110,
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            RecipeId::ClassicBurger => "Bun + fried patty + chopped lettuce/tomato",
            RecipeId::Cheeseburger => "Classic + cheese slice (chop cheese once)",
            RecipeId::DeluxeBurger => "Full works: fry + 3 chops",
            RecipeId::HeartyStew => "Chop carrot/potato/onion, boil pot 8s, ladle with bowl",
            RecipeId::SundayDinner => "Fry ham, oven potato 5s, chop carrot 2-stage",
            RecipeId::VeggieBurger => "Like Classic with veg patty",
            RecipeId::VeggieDeluxe => "Hardest: 2-stage chops + veg patty",
            RecipeId::NoodleBowl => "Flash-fry noodle brick 3s, chop onion+carrot, serve hot.",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            RecipeId::ClassicBurger => 0,
            RecipeId::Cheeseburger => 1,
            RecipeId::DeluxeBurger => 2,
            RecipeId::HeartyStew => 3,
            RecipeId::SundayDinner => 4,
            RecipeId::VeggieBurger => 5,
            RecipeId::VeggieDeluxe => 6,
            RecipeId::NoodleBowl => 7,
        }
    }

    pub fn dish_model(&self) -> &'static str {
        match self {
            RecipeId::ClassicBurger | RecipeId::Cheeseburger | RecipeId::DeluxeBurger => {
                "food_burger.gltf"
            }
            RecipeId::HeartyStew => "food_stew.gltf",
            RecipeId::SundayDinner => "food_dinner.gltf",
            RecipeId::VeggieBurger | RecipeId::VeggieDeluxe => "food_vegetableburger.gltf",
            RecipeId::NoodleBowl => "food_stew.gltf",
        }
    }
}

/// Does this exact set of plate contents match a recipe?
pub fn match_recipe(contents: &[Item]) -> Option<RecipeId> {
    let all = [
        RecipeId::ClassicBurger,
        RecipeId::Cheeseburger,
        RecipeId::DeluxeBurger,
        RecipeId::HeartyStew,
        RecipeId::SundayDinner,
        RecipeId::VeggieBurger,
        RecipeId::VeggieDeluxe,
        RecipeId::NoodleBowl,
    ];
    for r in all {
        if same_multiset(contents, r.components()) {
            return Some(r);
        }
    }
    None
}

fn same_multiset(a: &[Item], b: &[Item]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut counts: HashMap<Item, i32> = HashMap::new();
    for x in a {
        *counts.entry(*x).or_insert(0) += 1;
    }
    for x in b {
        *counts.entry(*x).or_insert(0) -= 1;
    }
    counts.values().all(|v| *v == 0)
}

pub struct ScriptedOrder {
    pub at_s: f32,
    pub recipe: RecipeId,
}

pub struct LevelConfig {
    pub index: usize,
    pub restaurant: &'static str,
    pub subtitle: &'static str,
    pub time_limit_s: f32,
    pub order_expire_s: f32,
    pub max_active_orders: usize,
    pub recipes: &'static [RecipeId],
    pub star_thresholds: (u32, u32, u32),
    pub script: &'static [ScriptedOrder],
}

pub const LEVELS: &[LevelConfig] = &[
    LevelConfig {
        index: 0,
        restaurant: "Burger Bistro",
        subtitle: "Learn the grill. (Step 1)",
        time_limit_s: 150.0,
        order_expire_s: 75.0,
        max_active_orders: 2,
        recipes: &[RecipeId::ClassicBurger, RecipeId::Cheeseburger],
        star_thresholds: (200, 400, 600),
        script: &[
            ScriptedOrder { at_s: 5.0, recipe: RecipeId::ClassicBurger },
            ScriptedOrder { at_s: 30.0, recipe: RecipeId::ClassicBurger },
            ScriptedOrder { at_s: 55.0, recipe: RecipeId::Cheeseburger },
            ScriptedOrder { at_s: 80.0, recipe: RecipeId::ClassicBurger },
            ScriptedOrder { at_s: 105.0, recipe: RecipeId::Cheeseburger },
            ScriptedOrder { at_s: 125.0, recipe: RecipeId::ClassicBurger },
        ],
    },
    LevelConfig {
        index: 1,
        restaurant: "Stew House",
        subtitle: "Chop, boil, Sunday roast. (Step 2)",
        time_limit_s: 210.0,
        order_expire_s: 90.0,
        max_active_orders: 3,
        recipes: &[RecipeId::HeartyStew, RecipeId::SundayDinner, RecipeId::NoodleBowl],
        star_thresholds: (400, 800, 1200),
        script: &[
            ScriptedOrder { at_s: 5.0, recipe: RecipeId::HeartyStew },
            ScriptedOrder { at_s: 25.0, recipe: RecipeId::SundayDinner },
            ScriptedOrder { at_s: 50.0, recipe: RecipeId::NoodleBowl },
            ScriptedOrder { at_s: 70.0, recipe: RecipeId::HeartyStew },
            ScriptedOrder { at_s: 95.0, recipe: RecipeId::SundayDinner },
            ScriptedOrder { at_s: 120.0, recipe: RecipeId::NoodleBowl },
            ScriptedOrder { at_s: 145.0, recipe: RecipeId::HeartyStew },
            ScriptedOrder { at_s: 170.0, recipe: RecipeId::SundayDinner },
            ScriptedOrder { at_s: 190.0, recipe: RecipeId::NoodleBowl },
        ],
    },
    LevelConfig {
        index: 2,
        restaurant: "Veggie Cafe",
        subtitle: "Full menu rush. (Step 3)",
        time_limit_s: 240.0,
        order_expire_s: 70.0,
        max_active_orders: 4,
        recipes: &[
            RecipeId::VeggieBurger,
            RecipeId::VeggieDeluxe,
            RecipeId::ClassicBurger,
            RecipeId::Cheeseburger,
            RecipeId::DeluxeBurger,
            RecipeId::HeartyStew,
            RecipeId::SundayDinner,
            RecipeId::NoodleBowl,
        ],
        star_thresholds: (550, 1100, 1650),
        script: &[
            ScriptedOrder { at_s: 5.0, recipe: RecipeId::VeggieBurger },
            ScriptedOrder { at_s: 20.0, recipe: RecipeId::ClassicBurger },
            ScriptedOrder { at_s: 40.0, recipe: RecipeId::VeggieDeluxe },
            ScriptedOrder { at_s: 60.0, recipe: RecipeId::HeartyStew },
            ScriptedOrder { at_s: 85.0, recipe: RecipeId::DeluxeBurger },
            ScriptedOrder { at_s: 110.0, recipe: RecipeId::VeggieBurger },
            ScriptedOrder { at_s: 130.0, recipe: RecipeId::Cheeseburger },
            ScriptedOrder { at_s: 150.0, recipe: RecipeId::VeggieDeluxe },
            ScriptedOrder { at_s: 170.0, recipe: RecipeId::NoodleBowl },
            ScriptedOrder { at_s: 185.0, recipe: RecipeId::SundayDinner },
            ScriptedOrder { at_s: 200.0, recipe: RecipeId::DeluxeBurger },
            ScriptedOrder { at_s: 215.0, recipe: RecipeId::NoodleBowl },
        ],
    },
];

pub fn score_serve(recipe: RecipeId, age_s: f32, expire_s: f32) -> u32 {
    let base = recipe.base_points();
    let remaining = (expire_s - age_s).max(0.0);
    let freshness = ((remaining / 10.0).floor() as u32) * 5;
    base + freshness
}

pub fn stars_for(score: u32, thresholds: (u32, u32, u32)) -> u8 {
    if score >= thresholds.2 {
        3
    } else if score >= thresholds.1 {
        2
    } else if score >= thresholds.0 {
        1
    } else {
        0
    }
}
