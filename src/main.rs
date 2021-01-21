#![allow(non_snake_case)]

use std::env::args;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use dirs_next::cache_dir;


const GAME_COLOR : &str = "\x1B[34m";
const PLAYTIME_COLOR : &str = "\x1B[0m";
const INFO_COLOR : &str = "\x1B[32m";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SteamOwnedGames {
    game_count: u32,
    games: Vec<SteamGame>
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
struct SteamGame {
    appid: u32,
    playtime_forever: u64,
    playtime_linux_forever: u64,
    playtime_mac_forever: u64,
    playtime_windows_forever: u64
}

struct SteamOwnedGamesWithNames {
    game_count: u32,
    games: Vec<SteamGameWithName>
}

struct SteamGameWithName {
    name: String,
    playtime_forever: u64,
    playtime_linux_forever: u64,
    playtime_mac_forever: u64,
    playtime_windows_forever: u64
}

impl SteamGameWithName {
    pub fn add_name(name: String, without_name: SteamGame) -> Self {
        SteamGameWithName {
            name,
            playtime_forever: without_name.playtime_forever,
            playtime_windows_forever: without_name.playtime_windows_forever,
            playtime_linux_forever: without_name.playtime_linux_forever,
            playtime_mac_forever: without_name.playtime_mac_forever
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SteamGameInfo {
    appid: u32,
    name: String
}

#[derive(Serialize, Deserialize)]
struct RawSteamGameInfo {
    apps: Vec<SteamGameInfo>
}

impl SteamGame {
    pub fn get_name(&self, data: &Vec<SteamGameInfo>) -> Result<String, ()> {
        for i in data {
            if i.appid == self.appid {
                return Ok(i.name.clone());
            }
        }
        return Err(());
    }
}

//noinspection SpellCheckingInspection
#[tokio::main]
async fn main() -> Result<(), reqwest::Error>{
    let user_id: u64 = args().nth(1).expect("Required: user_id").parse::<u64>().expect("user_id must be a number");
    let api_key : String = args().nth(2).expect("Required: api_key");
    let body = reqwest::get(format!("http://api.steampowered.com/IPlayerService/GetOwnedGames/v0001/?key={}&steamid={}&format=json", api_key,user_id).as_str())
        .await?
        .text()
        .await?;
    let body = body.strip_prefix("{\"response\":").expect("Invalid return json (wrong content)").strip_suffix("}").expect("Invalid return json (wrong_content)");
    let games_owned : SteamOwnedGames = serde_json::from_str(body).expect("Invalid return json (badly formatted)");
    let cached_file_path: PathBuf = cache_dir().expect("Err: cached dir not found").join("sui-cli-cached-steam-info.json");
    let app_names = get_names(&cached_file_path).await?;
    let (mut games_owned, _app_names) = add_names(&cached_file_path, games_owned, app_names).await;

    match args().nth(3) {
        Some(arg) => {
            match arg.as_str() {
                "playtime" => games_owned.games.sort_by(|x, y| {
                    x.playtime_forever.cmp(&y.playtime_forever)
                }),
                "name" => games_owned.games.sort_by(|x, y| {
                    x.name.cmp(&y.name)
                }),
                "default" => {},
                _ => println!("Invalid sort order. Continuing with default..."),
            }
        }
        None => {}
    }
    for i in games_owned.games {
        display_min_info(i.name, i.playtime_forever);
    }
    println!("{}  Total game count: {}", INFO_COLOR, games_owned.game_count);
    Ok(())
}

async fn get_names(cached_file_path : &PathBuf) -> Result<Vec<SteamGameInfo>, reqwest::Error> {
    let game_info = std::fs::read_to_string(cached_file_path);
    if game_info.is_err() {
        return Ok(refresh_names(cached_file_path).await?);
    }
    if let Ok(s) = serde_json::from_str(game_info.unwrap().as_str()) {
        return Ok(s);
    }
    return Ok(refresh_names(cached_file_path).await?);
}

async fn refresh_names(cached_file_path : &PathBuf) -> Result<Vec<SteamGameInfo>, reqwest::Error> {
    println!("Refreshing app names...");
    let app_names = reqwest::get("https://api.steampowered.com/ISteamApps/GetAppList/v2/")
        .await?
        .text()
        .await?;
    let app_names = app_names.strip_prefix("{\"applist\":").expect("Invalid AppList json (wrong content)").strip_suffix("}").expect("Invalid AppList json (wrong content)");

    let app_names: RawSteamGameInfo = serde_json::from_str(app_names).expect("Invalid AppList json (badly formatted)");
    std::fs::write(cached_file_path, serde_json::to_string(&app_names.apps).expect("Could not parse app_names (somehow)")).expect("Unable to write to file!");
    return Ok(app_names.apps);
}

fn display_min_info(name: String, playtime_forever: u64) {
    println!("{}{}: \n\t{}Playtime (total): {:.2}h", GAME_COLOR, name, PLAYTIME_COLOR, playtime_forever as f64/60.0);
}
fn display_names(name: String) {
    println!("{}{}", GAME_COLOR, name);
}

async fn add_names(cached_file_path : &PathBuf,without_names: SteamOwnedGames, app_names: Vec<SteamGameInfo>) -> (SteamOwnedGamesWithNames, Vec<SteamGameInfo>) {
    let mut games_with_names = Vec::with_capacity(without_names.games.len());
    let mut app_names = app_names;
    for i in without_names.games {
        match i.get_name(&app_names) {
            Err(_) => {
                app_names = refresh_names(cached_file_path).await.expect(format!["Unable to fetch new list, and old list is missing id {}. Aborting.", i.appid].as_str());
                if i.get_name(&app_names).is_err() {
                    panic!("Fetched new list, but it is still missing id {}!", i.appid);
                }
            }
            Ok(s) => {
                games_with_names.push(SteamGameWithName::add_name(s, i))
            }
        }
    }
    (SteamOwnedGamesWithNames {
        game_count: without_names.game_count,
        games: games_with_names
    }, app_names)
}