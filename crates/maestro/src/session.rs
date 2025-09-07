use std::{
    env,
    fs::{self, create_dir_all},
    io,
    path::PathBuf,
    sync::LazyLock,
};

use rand::{Rng, distr::Uniform, seq::IndexedRandom};

pub(crate) static SESSION_WORKDIR: LazyLock<Result<PathBuf, io::Error>> =
    LazyLock::new(setup_session_workdir);
fn setup_session_workdir() -> Result<PathBuf, io::Error> {
    let session_id = {
        let mut rng = rand::rng();
        let selected_adj = ADJECTIVES
            .choose(&mut rng)
            .expect("ADJECTIVES should not be empty for session ID selection!");
        let selected_animal = ANIMALS
            .choose(&mut rng)
            .expect("ANIMALS should not be empty for session ID selection!");
        format!("{}-{}", selected_adj, selected_animal)
    };
    let maestro_workdir = match env::var("MAESTRO_WORKDIR") {
        Ok(v) => PathBuf::from(v),
        Err(_) => env::current_dir()?.join("maestro_work"),
    };
    let session_workdir = maestro_workdir.join(&session_id);
    fs::create_dir_all(&session_workdir)?;
    println!(
        ":: New maestro session initialized!\n:: ID: {}\n:: Workdir: {}",
        session_id,
        session_workdir.display()
    );
    Ok(session_workdir)
}

pub(crate) fn create_process_dir() -> Result<PathBuf, io::Error> {
    fn generate_process_path() -> Result<PathBuf, io::Error> {
        let process_id: String = {
            let rng = rand::rng();
            let letter_sample = Uniform::new_inclusive('a', 'z')
                .expect("Uniform character sampling should not fail!");
            rng.sample_iter(letter_sample).take(8).collect()
        };
        let process_dir = SESSION_WORKDIR
            .as_ref()
            .map_err(|e| e.kind())?
            .join(process_id);
        Ok(process_dir)
    }
    let mut process_dir = generate_process_path()?;
    // For rare case where hashes are generated identically multiple times
    // Use bounded iterator; it is almost impossible for this to occur multiple times
    for _ in 0..3 {
        if !process_dir.exists() {
            break;
        }
        process_dir = generate_process_path()?;
    }
    if process_dir.exists() {
        panic!("Could not generate a unique process directory hash!")
    }
    create_dir_all(&process_dir)?;
    Ok(process_dir)
}

const ADJECTIVES: [&str; 100] = [
    "joyful",
    "grateful",
    "thrilled",
    "amused",
    "angry",
    "hopeful",
    "appreciative",
    "cheerful",
    "inspired",
    "sad",
    "affectionate",
    "proud",
    "enthusiastic",
    "elated",
    "content",
    "calm",
    "peaceful",
    "relaxed",
    "worried",
    "serene",
    "blissful",
    "exuberant",
    "radiant",
    "upbeat",
    "anxious",
    "cheery",
    "lively",
    "sunny",
    "bubbly",
    "vibrant",
    "delighted",
    "pleased",
    "frustrated",
    "mellow",
    "comical",
    "confident",
    "gracious",
    "accomplished",
    "satisfied",
    "stressed",
    "fulfilled",
    "happy",
    "harmonious",
    "sociable",
    "loving",
    "caring",
    "lonely",
    "compassionate",
    "empathetic",
    "friendly",
    "welcoming",
    "ecstatic",
    "jovial",
    "grumpy",
    "jubilant",
    "merry",
    "gleeful",
    "lighthearted",
    "carefree",
    "exhausted",
    "playful",
    "whimsical",
    "ambitious",
    "motivated",
    "determined",
    "focused",
    "irritated",
    "energized",
    "invigorated",
    "refreshed",
    "rejuvenated",
    "optimistic",
    "overwhelmed",
    "trustful",
    "bold",
    "courageous",
    "fearless",
    "animated",
    "disappointed",
    "spirited",
    "witty",
    "curious",
    "fascinated",
    "amazed",
    "gloomy",
    "astonished",
    "awed",
    "buoyant",
    "sentimental",
    "nostalgic",
    "bitter",
    "reflective",
    "thoughtful",
    "betrayed",
    "cynical",
    "miserable",
    "confused",
    "crushed",
    "jealous",
    "annoyed",
];

const ANIMALS: [&str; 100] = [
    "dog",
    "cow",
    "cat",
    "horse",
    "donkey",
    "tiger",
    "lion",
    "panther",
    "leopard",
    "cheetah",
    "bear",
    "elephant",
    "turtle",
    "tortoise",
    "crocodile",
    "rabbit",
    "porcupine",
    "hare",
    "hen",
    "pigeon",
    "crow",
    "fish",
    "dolphin",
    "frog",
    "whale",
    "alligator",
    "eagle",
    "ostrich",
    "fox",
    "goat",
    "jackal",
    "armadillo",
    "eel",
    "goose",
    "wolf",
    "gorilla",
    "chimpanzee",
    "monkey",
    "beaver",
    "orangutan",
    "antelope",
    "bat",
    "badger",
    "giraffe",
    "hamster",
    "cobra",
    "camel",
    "hawk",
    "deer",
    "chameleon",
    "hippopotamus",
    "jaguar",
    "lizard",
    "koala",
    "kangaroo",
    "iguana",
    "llama",
    "jellyfish",
    "rhinoceros",
    "hedgehog",
    "zebra",
    "possum",
    "wombat",
    "bison",
    "bull",
    "buffalo",
    "sheep",
    "meerkat",
    "mouse",
    "otter",
    "sloth",
    "owl",
    "vulture",
    "flamingo",
    "raccoon",
    "mole",
    "duck",
    "swan",
    "lynx",
    "elk",
    "boar",
    "lemur",
    "mule",
    "baboon",
    "mammoth",
    "snake",
    "peacock",
    "squirrel",
    "crab",
    "panda",
    "shark",
    "chinchilla",
    "pig",
    "penguin",
    "seal",
    "spider",
    "ant",
    "bee",
    "fly",
    "parrot",
];
