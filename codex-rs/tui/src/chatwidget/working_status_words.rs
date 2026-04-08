use std::time::Duration;

use rand::Rng as _;

pub(super) const DEFAULT_WORKING_STATUS: &str = "Working";
pub(super) const WORKING_STATUS_ROTATION_INTERVAL: Duration = Duration::from_secs(10);

const WORKING_STATUS_WORDS: &[&str] = &[
    "Accomplishing",
    "Actioning",
    "Actualizing",
    "Architecting",
    "Baking",
    "Beaming",
    "Beboppin'",
    "Befuddling",
    "Billowing",
    "Blanching",
    "Bloviating",
    "Boogieing",
    "Boondoggling",
    "Booping",
    "Bootstrapping",
    "Brewing",
    "Bunning",
    "Burrowing",
    "Calculating",
    "Canoodling",
    "Caramelizing",
    "Cascading",
    "Catapulting",
    "Cerebrating",
    "Channeling",
    "Channelling",
    "Choreographing",
    "Churning",
    "Clauding",
    "Coalescing",
    "Cogitating",
    "Combobulating",
    "Composing",
    "Computing",
    "Concocting",
    "Considering",
    "Contemplating",
    "Cooking",
    "Crafting",
    "Creating",
    "Crunching",
    "Crystallizing",
    "Cultivating",
    "Deciphering",
    "Deliberating",
    "Determining",
    "Dilly-dallying",
    "Discombobulating",
    "Doing",
    "Doodling",
    "Drizzling",
    "Ebbing",
    "Effecting",
    "Elucidating",
    "Embellishing",
    "Enchanting",
    "Envisioning",
    "Evaporating",
    "Fermenting",
    "Fiddle-faddling",
    "Finagling",
    "Flamb\u{e9}ing",
    "Flibbertigibbeting",
    "Flowing",
    "Flummoxing",
    "Fluttering",
    "Forging",
    "Forming",
    "Frolicking",
    "Frosting",
    "Gallivanting",
    "Galloping",
    "Garnishing",
    "Generating",
    "Gesticulating",
    "Germinating",
    "Gitifying",
    "Grooving",
    "Gusting",
    "Harmonizing",
    "Hashing",
    "Hatching",
    "Herding",
    "Honking",
    "Hullaballooing",
    "Hyperspacing",
    "Ideating",
    "Imagining",
    "Improvising",
    "Incubating",
    "Inferring",
    "Infusing",
    "Ionizing",
    "Jitterbugging",
    "Julienning",
    "Kneading",
    "Leavening",
    "Levitating",
    "Lollygagging",
    "Manifesting",
    "Marinating",
    "Meandering",
    "Metamorphosing",
    "Misting",
    "Moonwalking",
    "Moseying",
    "Mulling",
    "Mustering",
    "Musing",
    "Nebulizing",
    "Nesting",
    "Newspapering",
    "Noodling",
    "Nucleating",
    "Orbiting",
    "Orchestrating",
    "Osmosing",
    "Perambulating",
    "Percolating",
    "Perusing",
    "Philosophising",
    "Photosynthesizing",
    "Pollinating",
    "Pondering",
    "Pontificating",
    "Pouncing",
    "Precipitating",
    "Prestidigitating",
    "Processing",
    "Proofing",
    "Propagating",
    "Puttering",
    "Puzzling",
    "Quantumizing",
    "Razzle-dazzling",
    "Razzmatazzing",
    "Recombobulating",
    "Reticulating",
    "Roosting",
    "Ruminating",
    "Saut\u{e9}ing",
    "Scampering",
    "Schlepping",
    "Scurrying",
    "Seasoning",
    "Shenaniganing",
    "Shimmying",
    "Simmering",
    "Skedaddling",
    "Sketching",
    "Slithering",
    "Smooshing",
    "Sock-hopping",
    "Spelunking",
    "Spinning",
    "Sprouting",
    "Stewing",
    "Sublimating",
    "Swirling",
    "Swooping",
    "Symbioting",
    "Synthesizing",
    "Tempering",
    "Thinking",
    "Thundering",
    "Tinkering",
    "Tomfoolering",
    "Topsy-turvying",
    "Transfiguring",
    "Transmuting",
    "Twisting",
    "Undulating",
    "Unfurling",
    "Unravelling",
    "Vibing",
    "Waddling",
    "Wandering",
    "Warping",
    "Whatchamacalliting",
    "Whirlpooling",
    "Whirring",
    "Whisking",
    "Wibbling",
    "Working",
    "Wrangling",
    "Zesting",
    "Zigzagging",
];

pub(super) fn load_working_status_words() -> Vec<String> {
    WORKING_STATUS_WORDS
        .iter()
        .map(ToString::to_string)
        .collect()
}

pub(super) fn choose_working_status_word_index(words: &[String]) -> usize {
    if words.is_empty() {
        0
    } else {
        rand::rng().random_range(0..words.len())
    }
}

pub(super) fn working_status_word_for_index(words: &[String], index: usize) -> String {
    if words.is_empty() {
        return DEFAULT_WORKING_STATUS.to_string();
    }

    words[index].clone()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn loads_embedded_words() {
        let words = load_working_status_words();

        assert_eq!(words.first().map(String::as_str), Some("Accomplishing"));
        assert_eq!(words.last().map(String::as_str), Some("Zigzagging"));
        assert!(words.iter().any(|word| word == "Beboppin'"));
        assert!(words.iter().any(|word| word == "Working"));
    }

    #[test]
    fn falls_back_to_default_when_list_is_empty() {
        assert_eq!(
            working_status_word_for_index(&[], 0),
            DEFAULT_WORKING_STATUS
        );
    }

    #[test]
    fn returns_word_for_requested_index() {
        let words = vec![
            "Working".to_string(),
            "Thinking".to_string(),
            "Computing".to_string(),
        ];

        assert_eq!(working_status_word_for_index(&words, 1), "Thinking");
        assert_eq!(working_status_word_for_index(&words, 2), "Computing");
        assert_eq!(working_status_word_for_index(&words, 0), "Working");
    }
}
