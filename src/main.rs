#![allow(unused_mut)]
#![allow(unused_must_use)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_assignments)]
extern crate curl;
extern crate irc;
extern crate rusqlite;
extern crate rustc_serialize;
extern crate regex;
extern crate time;
extern crate rand;
extern crate crypto;
extern crate rss;
extern crate atom_syndication;

use std::env;
use std::thread;
use std::process::exit;
use std::str;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::OpenOptions;
use std::sync::mpsc::Sender;
use std::sync::mpsc;
use std::time::Duration;
use regex::Regex;
use curl::easy::{Easy, List};
use irc::client::prelude::*;
use rustc_serialize::json::Json;
use rusqlite::Connection;
use rand::Rng;
use self::crypto::digest::Digest;
use self::crypto::sha2::Sha512;
use rss::Rss;
use atom_syndication::Feed;

#[derive(Debug)]
struct BotConfig {
	nick: String,
	altn1: String,
	altn2: String,
	server: String,
	channel: String,
	prefix: String,
	admin: String,
	snpass: String,
	snuser: String,
	cookie: String,
	wu_key: String,
	go_key: String,
	bi_key: String,
	cse_id: String,
}

#[derive(Debug)]
struct CacheEntry {
	age: i64,
	location: String,
	weather: String,
}

struct Storables {
	wucache: Vec<CacheEntry>,
	botconfig: BotConfig,
	server: IrcServer,
	conn: Connection,
	titleres: Vec<Regex>,
	descres: Vec<Regex>,
}

#[derive(Debug)]
struct Submission {
	reskey: String,
	subject: String,
	story: String,
	chan: String,
	cookie: String,
	botnick: String,
}

const DEBUG: bool = false;
const ITEM_MONEY: i64 = 1_i64;
const RACES: &'static [&'static str] = &["human", "highelf", "woodelf", "darkelf", "hilldwarf", "mountaindwarf", "lightfoothalfling", "stouthalfling", "blackdragonborn", "bluedragonborn", "brassdragonborn", "bronzedragonborn", "copperdragonborn", "golddragonborn", "greendragonborn", "reddragonborn", "silverdragonborn", "whitedragonborn", "forestgnome", "rockgnome", "halfelf", "halforc", "tiefling"];
const CLASSES: &'static [&'static str] = &["barbarian", "bard", "cleric", "druid", "fighter", "monk", "paladin", "ranger", "rogue", "sorcerer", "warlock", "wizard"];

// status effects
const ABILITY_DARKVISION: u64 = 1_u64;
const ABILITY_SUPDARKVISION: u64 = 2_u64;
const ABILITY_DWFRESILIENCE: u64 = 4_u64;
const ABILITY_DWFCOMBATTRAIN: u64 = 8_u64;
const ABILITY_DWFTOOLPROF: u64 = 16_u64;
const ABILITY_DWFTOUGHNESS: u64 = 32_u64;
const ABILITY_DWFARMTRAIN: u64 = 64_u64;
const ABILITY_ELFKEENSENS: u64 = 128_u64;
const ABILITY_ELFFEYANCEST: u64 = 256_u64;
const ABILITY_ELFCOMBATTRAIN: u64 = 512_u64;
const ABILITY_ELFCANTRIP: u64 = 1024_u64;
const ABILITY_ELFMOTW: u64 = 2048_u64;
const ABILITY_DELFSUNBAD: u64 = 4096_u64;
const ABILITY_DELFMAGIC: u64 = 8192_u64;
const ABILITY_DELFWEAPON: u64 = 16384_u64;
const ABILITY_HFLLUCK: u64 = 32768_u64;
const ABILITY_HFLBRAVE: u64 = 65536_u64;
const ABILITY_HFLSTEALTH: u64 = 131072_u64;
const ABILITY_HFLRESILIENCE: u64 = 262144_u64;
const ABILITY_DBNFIRE: u64 = 524288_u64;
const ABILITY_DBNCOLD: u64 = 1048576_u64;
const ABILITY_DBNACID: u64 = 2097152_u64;
const ABILITY_DBNLIGHT: u64 = 4194304_u64;
const ABILITY_DBNPOIS: u64 = 8388608_u64;
const ABILITY_GNOCUNNING: u64 = 16777216_u64;
const ABILITY_GNOCANTRIP: u64 = 33554432_u64;
const ABILITY_HELSKILLPROF: u64 = 67108864_u64;
const ABILITY_HORMENACE: u64 = 134217728_u64;
const ABILITY_HORRELEND: u64 = 268435456_u64;
const ABILITY_HORSAVATKS: u64 = 536870912_u64;
const ABILITY_TIEFIRERES: u64 = 1073741824_u64;
const ABILITY_TIECANTRIP: u64 = 2147483648_u64;
const ABILITY_NONE: u64 = 4294967296_u64;


fn main() {
	let args: Vec<_> = env::args().collect();
	if args.len() < 2 {
		println!("Syntax: rustbot botnick");
		exit(0);
	}
	let thisbot = args[1].clone();
	let wucache: Vec<CacheEntry> = vec![];
	let conn = Connection::open("/home/bob/etc/snbot/usersettings.db").unwrap();
	let botconfig = conn.query_row("SELECT nick, server, channel, prefix, admin_hostmask, snpass, snuser, cookiefile, wu_api_key, google_key, bing_key, g_cse_id FROM bot_config WHERE nick = ?", &[&thisbot], |row| {
			BotConfig {
				nick: row.get(0),
				altn1: row.get(0),
				altn2: row.get(0),
				server: row.get(1),
				channel: row.get(2),
				prefix: row.get(3),
				admin: row.get(4),
				snpass: row.get(5),
				snuser: row.get(6),
				cookie: "".to_string(),
				wu_key: row.get(8),
				go_key: row.get(9),
				bi_key: row.get(10),
				cse_id: row.get(11),
			}
		}).unwrap();
	let mut storables: Storables = Storables {
		wucache: wucache,
		conn: Connection::open("/home/bob/etc/snbot/usersettings.db").unwrap(),
		botconfig: conn.query_row("SELECT nick, server, channel, prefix, admin_hostmask, snpass, snuser, cookiefile, wu_api_key, google_key, bing_key, g_cse_id FROM bot_config WHERE nick = ?", &[&thisbot], |row| {
			BotConfig {
				nick: row.get(0),
				altn1: row.get(0),
				altn2: row.get(0),
				server: row.get(1),
				channel: row.get(2),
				prefix: row.get(3),
				admin: row.get(4),
				snpass: row.get(5),
				snuser: row.get(6),
				cookie: "".to_string(),
				wu_key: row.get(8),
				go_key: row.get(9),
				bi_key: row.get(10),
				cse_id: row.get(11),
			}
			}).unwrap(),
		server: IrcServer::from_config(
				irc::client::data::config::Config {
					owners: None,
					nickname: Some(botconfig.nick.clone()),
					alt_nicks: Some(vec!(botconfig.altn1.clone(), botconfig.altn2.clone())),
					username: Some(botconfig.nick.clone()),
					realname: Some(botconfig.nick.clone()),
					server: Some(botconfig.server.clone()),
					port: Some(6697),
					password: Some(botconfig.snpass.clone()),
					use_ssl: Some(true),
					encoding: Some("UTF-8".to_string()),
					channels: Some(vec!(botconfig.channel.clone())),
					umodes: Some("+Zix".to_string()),
					user_info: Some("MrPlow rewritten in Rust".to_string()),
					ping_time: Some(180),
					ping_timeout: Some(10),
					ghost_sequence: Some(vec!("RECOVER".to_string())),
					should_ghost: Some(true),
					nick_password: Some(botconfig.snpass.clone()),
					options: None
			}).unwrap(),

		titleres: load_titleres(None),
		descres: load_descres(None),
	};
	storables.server.identify().unwrap();
	conn.close();

	// Spin off a submitter listening thread
	let (subtx, subrx) = mpsc::channel::<Submission>();
	{	
		let server = storables.server.clone();
		let substhread = thread::spawn(move || {
			loop {
			for submission in subrx.recv() {
				if DEBUG {
					println!("{:?}", submission);
				}
				thread::sleep(Duration::new(25,0));
				let chan = submission.chan.clone();
				if send_submission(&submission) {
					server.send_privmsg(&chan, "Submission successful. https://soylentnews.org/submit.pl?op=list");
				}
				else {
					server.send_privmsg(&chan, "Something borked during submitting, check the logs.");
				}
			}
			}
		});
	}

	for message in storables.server.iter() {
		let umessage = message.unwrap();
		let mut chan: String = "foo".to_string();
		let mut said: String = "bar".to_string();
		let msgclone = umessage.clone();
		let nick = msgclone.source_nickname();
		let snick: String;
		match umessage.command {
			irc::client::data::command::Command::PRIVMSG(ref c, ref d) => {
				splitprivmsg(&mut chan, &mut said, &c, &d);
				said = said.trim_right().to_string();
				let hostmask = msgclone.prefix.clone().unwrap().to_string();
				snick = nick.unwrap().to_string();
				println!("{:?}", umessage);

				if check_messages(&storables.conn, &snick) {
					deliver_messages(&storables.server, &storables.conn, &snick);
				}

				if is_action(&said) {
					let mut asaid = said.clone();
					asaid = asaid[8..].to_string();
					let asaidend = asaid.len() - 1;
					asaid = asaid[..asaidend].to_string();
					log_seen(&storables, &chan, &snick, &hostmask, &asaid, 1);
					process_action(&storables, &snick, &chan, &said);
				}
				else if is_command(&mut storables.botconfig.prefix, &said) {
					process_command(&mut storables.titleres, &mut storables.descres, &storables.server, &subtx, &storables.conn, &mut storables.wucache, &mut storables.botconfig, &snick, &hostmask, &chan, &said);
					log_seen(&storables, &chan, &snick, &hostmask, &said, 0);
				}
				else {
					log_seen(&storables, &chan, &snick, &hostmask, &said, 0);
					continue;
				}
			},
			irc::client::data::command::Command::PING(_,_) => continue,
			_ => println!("{:?}", umessage)
		}
	}
}

fn splitprivmsg(chan: &mut String, said: &mut String, c: &String, d: &String) {
	*chan = c.clone();
	*said = d.clone();
}

fn is_action(said: &String) -> bool {
	let prefix = "\u{1}ACTION ".to_string();
	let prefixbytes = prefix.as_bytes();
	let prefixlen = prefixbytes.len();
	let saidbytes = said.as_bytes();
	if prefix.len() > said.len() {
		return false;
	}
	let checkbytes = &saidbytes[0..prefixlen];
	if prefixbytes == checkbytes {
		return true;
	}
	false
}

fn is_command(prefix: &String, said: &String) -> bool {
	if said.len() < prefix.len() {
		return false;
	}
	let prefixlen = prefix.len();
	let prefixbytes = prefix.as_bytes();
	let saidbytes = said.as_bytes();
	let checkbytes = &saidbytes[0..prefixlen];
	if prefixbytes == checkbytes {
		return true;
	}
	false
}

fn log_seen(storables: &Storables, chan: &String, snick: &String, hostmask: &String, said: &String, action: i32) {
	let conn = &storables.conn;
	let time: i64 = time::now_utc().to_timespec().sec;
	conn.execute("REPLACE INTO seen VALUES($1, $2, $3, $4, $5, $6)", &[snick, hostmask, chan, said, &time, &action]).unwrap();
}

fn process_action(storables: &Storables, nick: &String, channel: &String, said: &String) {
	let server = &storables.server;
	let prefix = "\u{1}ACTION ".to_string();
	let prefixlen = prefix.len();
	let end = said.len() - 1;
	let csaid: String = said.clone();
	let action: String = csaid[prefixlen..end].to_string();
	if action == "yawns" {
		let flip = format!("flips a Skittle into {}'s gaping mouth", nick);
		server.send_action( channel, &flip );
	}
}

fn process_command(mut titleres: &mut Vec<Regex>, mut descres: &mut Vec<Regex>, server: &IrcServer, subtx: &Sender<Submission>, conn: &Connection, mut wucache: &mut Vec<CacheEntry>, mut botconfig: &mut BotConfig, nick: &String, hostmask: &String, chan: &String, said: &String) {
	let maskonly = hostmask_only(&hostmask);
	let prefix = botconfig.prefix.clone();
	let prefixlen = prefix.len();
	let saidlen = said.len();
	let csaid: String = said.clone();
	let noprefix: String = csaid[prefixlen..saidlen].to_string().trim().to_string();
	let noprefixbytes = noprefix.as_bytes();
	if noprefix.len() > 3 && &noprefixbytes[..4] == "quit".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		command_quit(server, chan.to_string());
	}
	if noprefix.len() > 6 && &noprefixbytes[..7] == "pissoff".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		command_pissoff(server, chan.to_string());
	}
	if noprefix.len() > 9 && &noprefixbytes[..10] == "dieinafire".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		command_dieinafire(server, chan.to_string());
	}
	else if noprefix.len() > 3 && &noprefixbytes[..4] == "join".as_bytes() {
		if is_abuser(&server, &conn, &chan, &maskonly) {
			return;
		}
		if noprefix.trim().len() < 7 {
			return;
		}
		if noprefix[5..6].to_string() != "#".to_string() {
			return;
		}
		let joinchan: String = noprefix[4..].to_string();
		command_join(&server, joinchan);
	}
	else if noprefix.len() > 3 && &noprefixbytes[..4] == "seen".as_bytes() {
		if noprefix.trim().len() < 6 {
			return;
		}
		let who: String = noprefix[4..].to_string().trim().to_string();
		command_seen(&server, &conn, &chan, who);
	}
	else if noprefix.len() > 7 && &noprefixbytes[..8] == "smakeadd".as_bytes() {
		if is_abuser(&server, &conn, &chan, &maskonly) {
			return;
		}
		if noprefix.trim().len() < 10 {
			return;
		}
		let what: String = noprefix[8..].to_string().trim().to_string();
		command_smakeadd(&server, &conn, &chan, what);
	}
	else if noprefix.len() > 4 && &noprefixbytes[..5] == "smake".as_bytes() {
		if noprefix.trim().len() < 7 {
			return;
		}
		let who: String = noprefix[5..].trim().to_string();
		command_smake(&server, &conn, &chan, who);
	}
	else if noprefix.len() > 9 && &noprefixbytes[..10] == "weatheradd".as_bytes() {
		if noprefix.len() < 16 {
			return;
		}
		let checklocation: String = noprefix[10..].trim().to_string();
		command_weatheradd(&server, &conn, &nick, &chan, checklocation);
	}
	else if noprefix.len() > 6 && &noprefixbytes[..7] == "weather".as_bytes() {
		if is_abuser(&server, &conn, &chan, &maskonly) {
			return;
		}
		let checklocation: Option<String>;
		if noprefix.trim().len() == 7 {
			checklocation = None;
		}
		else {
			checklocation = Some(noprefix[7..].trim().to_string());
		}
		command_weather(&botconfig, &server, &conn, &mut wucache, &nick, &chan, checklocation);
		return;
	}
	else if noprefix.len() > 5 && &noprefixbytes[..6] == "abuser".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		if noprefix.trim().len() < 8 {
			return;
		}
		let abuser: String = noprefix[6..].trim().to_string();
		command_abuser(&server, &conn, &chan, abuser);
		return;
	}
	else if noprefix.len() > 2 && &noprefixbytes[..3] == "bot".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		if noprefix.trim().len() < 5 {
			return;
		}
		let abuser: String = noprefix[4..].trim().to_string();
		command_bot(&server, &conn, &chan, abuser);
		return;
	}
	else if noprefix.len() > 4 && &noprefixbytes[..5] == "admin".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		if noprefix.trim().len() < 7 {
			return;
		}
		let abuser: String = noprefix[5..].trim().to_string();
		command_admin(&server, &conn, &chan, abuser);
		return;
	}
	else if noprefix.len() > 5 && &noprefixbytes[..6] == "submit".as_bytes() {
		if is_abuser(&server, &conn, &chan, &maskonly) {
			return;
		}
		if noprefix.find("http").is_none() {
			return;
			//help("submit");
		}
		let (suburl, summary) = sub_parse_line(&noprefix);
		command_submit(&mut botconfig, titleres, descres, &server, &chan, &subtx, suburl, summary, &nick);
	}
	else if noprefix.len() > 3 && &noprefixbytes[..4] == "help".as_bytes() {
		let command: Option<String>;
		if noprefix.trim().len() == 4 {
			command = None;
		}
		else {
			command = Some(noprefix[4..].to_string().trim().to_string());
		}
		command_help(&server, &botconfig, &chan, command);
	}
	else if noprefix.len() > 6 && &noprefixbytes[..7] == "youtube".as_bytes() {
		if is_abuser(&server, &conn, &chan, &maskonly) {
			return;
		}
		if noprefix.len() < 9 {
			server.send_privmsg(&chan, get_help(&botconfig.prefix, Some("youtube".to_string())).as_str());
			return;
		}
		let query: String = noprefix[7..].to_string().trim().to_string();
		command_youtube(&server, &botconfig, &chan, query);
	}
	else if noprefix.len() > 8 && &noprefixbytes[..9] == "socialist".as_bytes() {
		
		server.send_privmsg(&chan, format!("{}, you're a socialist!", &noprefix[9..].trim()).as_str());
	}
	else if noprefix.len() > 3 && &noprefixbytes[..4] == "roll".as_bytes() {
		if noprefix.len() < 8 {
			command_help(&server, &botconfig, &chan, Some("roll".to_string()));
			return;
		}
		let args = noprefix[4..].trim().to_string();
		command_roll(&server, &botconfig, &chan, args);
	}
	else if noprefix.len() > 2 && &noprefixbytes[..3] == "bnk".as_bytes() {
		server.send_privmsg(&chan, "https://www.youtube.com/watch?v=9upTLWRZTfw");
	}
	else if noprefix.len() > 3 && &noprefixbytes[..4] == "part".as_bytes() {
		if noprefix.len() == 4 {
			let partchan = chan.clone();
			command_part(&server, partchan);
		}
		else if noprefix.len() > 6 {
			let mut partchan = noprefix[4..].trim().to_string();
			let sp = partchan.find(" ");
			if sp.is_some() {
				let end = sp.unwrap();
				partchan = partchan[..end].trim().to_string();
			}
			command_part(&server, partchan);
		}
		else {
			server.send_privmsg(&chan, "Stop that.");
		}
	}
	else if noprefix.len() > 2 && &noprefixbytes[..3] == "say".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		let mut space = noprefix.find(" ").unwrap_or(0);
		let nocommand = noprefix[space..].trim().to_string();
		space = nocommand.find(" ").unwrap_or(0);
		let channel = nocommand[..space].trim().to_string();
		let message = nocommand[space..].trim().to_string();
		command_say(&server, channel, message);
	}
	else if noprefix.len() > 3 && &noprefixbytes[..4] == "tell".as_bytes() {
		let space = noprefix.find(" ").unwrap_or(0);
		if space == 0 { return; }
		let nocommand = noprefix[space..].trim().to_string();
		command_tell(&server, &conn, &chan, &nick, nocommand);
	}
	else if noprefix.len() > 6 && &noprefixbytes[..7] == "klingon".as_bytes() {
		if noprefix.len() < 9 {
			command_help(&server, &botconfig, &chan, Some("klingon".to_string()));
			return;
		}
		let english = noprefix[7..].trim().to_string();
		command_klingon(&server, &botconfig, &chan, english);
	}
	else if noprefix.len() == 1 && &noprefixbytes[..] == "g".as_bytes() {
		command_help(&server, &botconfig, &chan, Some("g".to_string()));
		return;
	}
	else if noprefix.len() > 2  && &noprefixbytes[..2] == "g ".as_bytes() {
		let searchstr = noprefix[1..].trim().to_string();
		command_google(&server, &botconfig, &chan, searchstr);
	}
	else if noprefix.len() == 9 && &noprefixbytes[..9] == "character".as_bytes() {
		command_help(&server, &botconfig, &chan, Some("character".to_string()));
	}
	else if noprefix.len() > 9 && &noprefixbytes[..10] == "character ".as_bytes() {
		let command = noprefix[10..].trim().to_string();
		command_character(&server, &botconfig, &conn, &chan, &nick, command);
	}
	else if &noprefixbytes[..] == "reloadregexes".as_bytes() {
		*titleres = load_titleres(None);
		*descres = load_descres(None);
	}
	else if noprefix.len() > 10 && &noprefixbytes[..11] == "sammichadd ".as_bytes() {
		if is_abuser(&server, &conn, &chan, &maskonly) {
			return;
		}
		let sammich = noprefix[11..].trim().to_string();
		command_sammichadd(&server, &botconfig, &conn, &chan, sammich);
	}
	else if noprefix.len() > 8 && &noprefixbytes[..8] == "sammich ".as_bytes() {
		command_sammich_alt(&server, &chan, &noprefix[8..].to_string().trim().to_string());
		return;
	}
	else if noprefix.len() > 6 && &noprefixbytes[..7] == "sammich".as_bytes() {
		command_sammich(&server, &botconfig, &conn, &chan, &nick);
	}
	else if noprefix.len() > 5 && &noprefixbytes[..] == "nelson".as_bytes() {
		let message = "HA HA!".to_string();
		command_say(&server, chan.to_string(), message);
	}
	else if noprefix.len() > 7 && &noprefixbytes[..7] == "nelson ".as_bytes() {
		let target = noprefix[7..].trim().to_string();
		let message = format!("{}: HA HA!", &target);
		command_say(&server, chan.to_string(), message);
	}
	else if noprefix.len() == 7 && &noprefixbytes[..] == "feedadd".as_bytes() {
		command_help(&server, &botconfig, &chan, Some("feedadd".to_string()));
	}
	else if noprefix.len() > 8 && &noprefixbytes[..8] == "feedadd ".as_bytes() {
		if !is_admin(&botconfig, &server, &conn, &chan, &maskonly) {
			return;
		}
		let feed_url = noprefix[7..].to_string().trim().to_string();
		command_feedadd(&server, &botconfig, &conn, &chan, feed_url);
	}
}

fn command_feedadd(server: &IrcServer, botconfig: &BotConfig, conn: &Connection, chan: &String, feed_url: String) {
	if !sql_table_check(&conn, "feeds".to_string()) {
		println!("`feeds` table not found, creating...");
		if !sql_table_create(&conn, "feeds".to_string()) {
			server.send_privmsg(&chan, "No feeds table exists and for some reason I cannot create one");
			return;
		}
	}

	let raw_feed = get_raw_feed(&feed_url);
	let feed_title = get_feed_title(raw_feed);
	if &feed_title[..] == "Unknown feed type" {
		server.send_privmsg(&chan, "Unknown feed type. RSS v1.0 maybe?");
		return;
	}
	
	match conn.execute("INSERT INTO feeds (title, address, frequency, lastchecked) VALUES($1, $2, 15, datetime('now', '-16 minutes'))", &[&feed_title, &feed_url]) {
		Err(err) => {
			println!("{}", err);
			server.send_privmsg(&chan, "Error writing to feeds table.");
			return;
		},
		Ok(_) => {
			let sayme: String = format!("\"{}\" added.", feed_url);
			server.send_privmsg(&chan, &sayme);
			return;
		},
	};
}

fn command_sammichadd(server: &IrcServer, botconfig: &BotConfig, conn: &Connection, chan: &String, sammich: String) {
	if !sql_table_check(&conn, "sammiches".to_string()) {
		println!("`sammiches` table not found, creating...");
		if !sql_table_create(&conn, "sammiches".to_string()) {
			server.send_privmsg(&chan, "No sammiches table exists and for some reason I cannot create one");
			return;
		}
	}
	
	match conn.execute("INSERT INTO sammiches VALUES(NULL, $1)", &[&sammich]) {
		Err(err) => {
			println!("{}", err);
			server.send_privmsg(&chan, "Error writing to sammiches table.");
			return;
		},
		Ok(_) => {
			let sayme: String = format!("\"{}\" added.", sammich);
			server.send_privmsg(&chan, &sayme);
			return;
		},
	};
}

fn command_sammich(server: &IrcServer, botconfig: &BotConfig, conn: &Connection, chan: &String, nick: &String) {
	if !sql_table_check(&conn, "sammiches".to_string()) {
		println!("`sammiches` table not found, creating...");
		if !sql_table_create(&conn, "sammiches".to_string()) {
			server.send_privmsg(&chan, "No sammiches table exists and for some reason I cannot create one");
			return;
		}
	}

	let check: i32 = conn.query_row("select count(*) from sammiches", &[], |row| {
		row.get(0)
	}).unwrap();
	if check == 0 {
		server.send_privmsg(&chan, "No sammiches in the database, add some.");
	}

	let result: String = conn.query_row("select sammich from sammiches order by random() limit 1", &[], |row| {
		row.get(0)
	}).unwrap();

	let dome: String = format!("whips up a {} sammich for {}", result, nick);
	server.send_action(&chan, &dome);
}

fn command_sammich_alt(server: &IrcServer, chan: &String, target: &String) {
	if is_nick_here(&server, &chan, &target) {
		let sneak = format!("sneaks up behind {} and cuts their throat", &target);
		let makesammich = format!("fixes thinly sliced {}'s corpse sammiches for everyone in {}", &target, &chan);
		server.send_action(&chan, &sneak.as_str());
		server.send_action(&chan, &makesammich.as_str());
		return;
	}
	else {
		let action = format!("looks around but does not see {}", &target);
		server.send_action(&chan, &action);
		return;
	}
}

fn command_character(server: &IrcServer, botconfig: &BotConfig, conn: &Connection, chan: &String, nick: &String, command: String) {
	if is_ns_faker(&server, &nick) {
		server.send_privmsg(&chan, "Sorry, you have to be registered and identified with nickserv to play. /ns help");
		return;
	}

	let commandbytes = command.as_bytes();

	if !character_exists(&conn, &nick) && command != "new".to_string() {
		let msg = format!("You need to register a character first with {}character new.", &botconfig.prefix);
	}
	// #character new
	else if command == "new".to_string() {
		let stats = roll_stats(&conn, &nick);
		let statstr = format!("Str: {}, Con: {}, Dex: {}, Int: {}, Wis: {}, Cha: {}", stats[0], stats[1], stats[2], stats[3], stats[4], stats[5]);
		let msg = format!("Stats rolled ({}), now pick your race then class. {}character race <race> then {}character class <class>. Use 'list' as the race or class to see the availible choices.", statstr, &botconfig.prefix, &botconfig.prefix);
		server.send_privmsg(&nick, &msg);
	}
	// #character race
	else if command.len() > 4 && &commandbytes[..5] == "race ".as_bytes() {
		let race = command[5..].trim().to_string();
		if race.len() < 1 { return; }
		else if race == "list".to_string() {
			let races = list_races();
			let sayme = races.join(", ").to_string();
			server.send_privmsg(&chan, &sayme);
		}
		else if is_race_set(&conn, &nick) {
			server.send_privmsg(&chan, "You've already picked a race.");
		}
		else if race.find(" ").is_some() {
			server.send_privmsg(&chan, "Stop that.");
		}
		else if is_valid_race(&race) {
			set_race(&conn, &nick, &race);
			let (strength, con, dex, int, wis, cha, abilitiesi) = conn.query_row("SELECT * FROM players WHERE nick = $1", &[&nick.as_str()], |row| {
				( row.get(5), row.get(6), row.get(7), row.get(8), row.get(9), row.get(10), row.get::<i64>(11) )
			}).unwrap_or( (0_i64, 0_i64, 0_i64, 0_i64, 0_i64, 0_i64, 0_i64) );
			let abilities: u64 = abilitiesi as u64;
			let abstr: String = read_abilities(abilities);
			let mut msg = format!("Race for {} set to {}.", &nick, &race);
			server.send_privmsg(&chan, &msg);
			msg = format!("Stats now Str: {}, Con: {}, Dex: {}, Int: {}, Wis: {}, Cha: {}. Abilities: {}", &strength, &con, &dex, &int, &wis, &cha, &abstr);
			server.send_privmsg(&nick, &msg);
		}
		else {
			let msg = format!("'{}' is not a valid race. Use '{}character race list' for valid choices.", &race, &botconfig.prefix);
			server.send_privmsg(&chan, &msg);
		}
	}
	// #character class
	else if command.len() > 5 && &commandbytes[..6] == "class ".as_bytes() {
		let class = command[6..].trim().to_string();
		if class.len() < 1 { return;}
		else if class == "list".to_string() {
			let classes = list_classes();
			let sayme = classes.join(", ").to_string();
			server.send_privmsg(&chan, sayme.as_str());
		}
		else if is_class_set(&conn, &nick) {
			server.send_privmsg(&chan, "You've already picked a class.");
		}
		else if class.find(" ").is_some() {
			server.send_privmsg(&chan, "Stop that.");
		}
		else if is_valid_class(&class) {
			set_class(&conn, &nick, &class);
			let msg = format!("Class for {} set to {}.", &nick, &class);
			server.send_privmsg(&chan, &msg);
		}
		else {
			let msg = format!("'{}' is not a valid class. Use '{}character class list' for valid choices.", &class, &botconfig.prefix);
			server.send_privmsg(&chan, &msg);
		}
	}
	// #character info
	else if command.len() > 3 && &commandbytes[..4] == "info".as_bytes() {
		let player = command[4..].trim().to_string();
		if player.len() < 1 { return; }
		else if *nick == player && character_exists(&conn, &player) {
			// full info display
			let info = display_player_info(&conn, &player, true);
			server.send_privmsg(&nick, &info);
		}
		else if player.find(" ").is_some() {
			server.send_privmsg(&chan, "Stop that.");
		}
		else if character_exists(&conn, &player) {
			// public info display
			let info = display_player_info(&conn, &player, false);
			server.send_privmsg(&chan, &info);
		}
		else {
			let msg = format!("Sorry, {} has not yet registered for the game.", &player);
			server.send_privmsg(&chan, &msg);
		}
	}
	else { command_help(&server, &botconfig, &chan, Some("character".to_string())); }
	return;
}

fn command_google(server: &IrcServer, botconfig: &BotConfig, chan: &String, searchstr: String) {
	let mut dst = Vec::new();
	{
		let mut easy = Easy::new();
		let bsearchstr = &searchstr.clone().into_bytes();
		let esearchstr = easy.url_encode(&bsearchstr[..]);
		let bcx = &botconfig.cse_id.clone().into_bytes();
		let ecx = easy.url_encode(&bcx[..]);
		let bkey = &botconfig.go_key.clone().into_bytes();
		let ekey = easy.url_encode(&bkey[..]);
		let url = format!("https://www.googleapis.com/customsearch/v1?q={}&cx={}&safe=off&key={}", esearchstr, ecx, ekey);

		easy.url(url.as_str()).unwrap();
		easy.write_function(|data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		});
		easy.perform().unwrap();
		if easy.response_code().unwrap_or(999) != 200 {
			println!("got http response code {} in command_google", easy.response_code().unwrap_or(999));
			return;
		}
	}
	let json = str::from_utf8(&dst[..]).unwrap_or("");
	let jsonthing = Json::from_str(json).unwrap();
	if jsonthing.find("items").is_none() {
		server.send_privmsg(&chan, "sorry, there were no results for your query");
		return;
	}
	let items = jsonthing.find("items").unwrap();
	let mut resurl = items[0].find("link").unwrap().to_string().trim().to_string();
	let mut ressum = items[0].find("snippet").unwrap().to_string().trim().to_string(); 
	if &resurl[0..1] == "\"" {
		let cresurl = resurl.clone();
		let strresurl = cresurl.as_str();
		let len = strresurl.len() - 1;
		resurl = strresurl[1..len].to_string().trim().to_string();
	}
	let regex = Regex::new(r"\\n").unwrap();
	ressum = regex.replace_all(ressum.as_str(), "");
	let response = format!("{} - {}", resurl, ressum);
	server.send_privmsg(&chan, &response);
}

fn command_klingon(server: &IrcServer, botconfig: &BotConfig, chan: &String, english: String) {
	let token = get_bing_token(&botconfig);
	if token == "".to_string() {
		server.send_privmsg(&chan, "Could not get bing translate token, check the logs");
		return;
	}
	let outlangs = vec!["tlh", "tlh-Qaak"];
	let mut dst = Vec::new();
	let mut translations = Vec::new();
	for lang in outlangs.iter() {
		let mut headerlist = List::new();
		headerlist.append(format!("Authorization: Bearer {}", token).as_str());
		headerlist.append("Accept-Language: en-US");
		headerlist.append("Accept-Charset: utf-8");
		{
			let mut easy = Easy::new();
			let benglish = &english.clone().into_bytes();
			let eenglish = easy.url_encode(&benglish[..]);
			let url = format!("http://api.microsofttranslator.com/V2/Http.svc/Translate?text={}&from=en&to={}&contentType=text/plain", &eenglish, &lang);
			easy.url(url.as_str()).unwrap();
			easy.http_headers(headerlist).unwrap();
			easy.write_function(|data: &[u8]| {
				dst.extend_from_slice(data);
				data.len()
			});
			easy.perform().unwrap();
			if easy.response_code().unwrap_or(999) != 200 {
				println!("got http response code {} in command_klingon", easy.response_code().unwrap_or(999));
				return;
			}
		}
		let cdst = dst.clone();
		let translation = String::from_utf8(cdst).unwrap_or("".to_string());
		dst = Vec::new();
		translations.push(translation);
	}
	for this in translations.iter() {
		println!("{}", this);
	}
	let reg = Regex::new("^<string.*>(.*?)</string>").unwrap();
	let capone = reg.captures(translations[0].as_str());
	let captwo = reg.captures(translations[1].as_str());
	let tlh = capone.unwrap().at(1).unwrap_or("wtf?!");
	let qaak = captwo.unwrap().at(1).unwrap_or("wtf?!");
	server.send_privmsg(&chan, format!("{} ({})    ", tlh, qaak).as_str());
	return;
}

fn command_tell(server: &IrcServer, conn: &Connection, chan: &String, nick: &String, incoming: String) {
	let space = incoming.find(" ").unwrap_or(0);
	if space == 0 { return; }
	let tellwho = incoming[..space].trim().to_string();
	let tellwhat = incoming[space..].trim().to_string();
	if tellwho.len() < 1 || tellwhat.len() < 1 {
		return;
	}
	if save_msg(&conn, &nick, tellwho, tellwhat) {
		server.send_privmsg(&chan, "Okay, I'll tell them next time I see them.");
	}
	else {
		server.send_privmsg(&chan, "Something borked saving your message, check the logs.");
	}
	return;
}

fn command_roll(server: &IrcServer, botconfig: &BotConfig, chan: &String, args: String) {
	let maxdice = 9000;
	let maxsides = 9000;
	let maxthrows = 13;
	let mut rng = rand::thread_rng();
	let regone = Regex::new("(\\d+)(?i:d)(\\d+)").unwrap();
	let regtwo = Regex::new("throws=(\\d+)").unwrap();
	let captureone = regone.captures(args.as_str());
	let capturetwo = regtwo.captures(args.as_str());
	let mut throws = "1";
	if captureone.is_none() {
		command_help(&server, &botconfig, &chan, Some("roll".to_string()));
		return;
	}
	if capturetwo.is_some() {
		throws = capturetwo.unwrap().at(1).unwrap_or("1");
		println!("throws: {}", throws);
	}
	else {
	}
	let throw: u64 = throws.parse::<u64>().unwrap_or(0) + 1_u64;
	let capture = captureone.unwrap();
	let dices = capture.at(1).unwrap_or("0");
	let dice: u64 = dices.parse::<u64>().unwrap_or(0) + 1_u64;
	let sides = capture.at(2).unwrap_or("0");
	let side: u64 = sides.parse().unwrap_or(0);
	if side > maxsides || dice > maxdice || throw > maxthrows {
		server.send_privmsg(&chan, format!("chromas, is that you? stop being a wiseass.").as_str());
		return;
	}
	else if side < 1 || dice < 1 || throw < 1 {
		server.send_privmsg(&chan, format!("chromas, is that you? stop being a wiseass.").as_str());
                return;
	}

	for pass in 1..throw {
		let mut total: u64 = 0_u64;
		for _ in 1..dice {
			let bignum = rng.gen::<u64>();
			let thisdie = (bignum % side) + 1;
			total += thisdie;
		}
		server.send_privmsg(&chan, format!("pass {}: {}", pass, total).as_str());
	}
	return;
}

fn command_youtube(server: &IrcServer, botconfig: &BotConfig, chan: &String, query: String) {
	let link = get_youtube(&botconfig.go_key, &query);
	server.send_privmsg(&chan, format!("https://www.youtube.com/watch?v={}", link).as_str());
	return;
}

fn command_submit(mut botconfig: &mut BotConfig, mut titleres: &mut Vec<Regex>,mut descres: &mut Vec<Regex>, server: &IrcServer, chan: &String, subtx: &Sender<Submission>, suburl: String, summary: String, submitter: &String) {
	let page: String = sub_get_page(&suburl);
	let title: String = sub_get_title(&mut titleres, &page);
	if title == "".to_string() {
		server.send_privmsg(&chan, "Unable to find a title for that page");
		return;
	}

	let description: String = sub_get_description(&mut descres, &page);
	if description == "".to_string() {
		server.send_privmsg(&chan, "Unable to find a summary for that page");
		return;
	}
	
	let mut cookie = botconfig.cookie.clone();
	if cookie == "".to_string() {
		cookie = sub_get_cookie(&mut botconfig);
	}
	
	let reskey = sub_get_reskey(&cookie);
	if reskey == "".to_string() {
		server.send_privmsg(&chan, "Unable to get a reskey. Check the logs.");
		return;
	}
	
	let story = sub_build_story( &submitter, &description, &summary, &suburl );
	
	let submission = Submission {
		reskey: reskey.clone(),
		subject: title.clone(),
		story: story.clone(),
		chan: chan.clone(),
		cookie: cookie.clone(),
		botnick: botconfig.nick.clone(),
	};

	server.send_privmsg(&chan, "Submitting. There is a mandatory delay, please be patient.");
	
	let foo = subtx.send(submission);
	match foo {
		Ok(_) => {},
		Err(err) => println!("{:?}", err),
	};
}

fn command_quit(server: &IrcServer, chan: String) {
	server.send_privmsg(&chan, "Your wish is my command...");
	server.send_quit("");
}

fn command_pissoff(server: &IrcServer, chan: String) {
	server.send_privmsg(&chan, "Off I shall piss...");
	server.send_quit("");
}

fn command_dieinafire(server: &IrcServer, chan: String) {
	server.send_action(&chan, "dies a firey death");
	server.send_quit("");
}

fn command_join(server: &IrcServer, joinchan: String) {
	server.send_join(&joinchan);
}

fn command_part(server: &IrcServer, partchan: String) {
	let botconfig = server.config();
	let channels = botconfig.clone().channels.unwrap();
	let homechannel = channels[0].clone();
	if homechannel.to_string() != partchan {
		let partmsg: Message = Message {
			tags: None,
			prefix: None,
			command: Command::PART(partchan, None), 
		};
		server.send(partmsg);
		return;
	}
	
}

fn command_say(server: &IrcServer, chan: String, message: String) {
	server.send_privmsg(&chan, message.as_str());
	return;
}

fn command_seen(server: &IrcServer, conn: &Connection, chan: &String, who: String) {
	struct SeenResult {
		channel: String,
		said: String,
		datetime: String,
		nick: String,
		action: bool
	};
	let result: SeenResult;
	if !sql_table_check(&conn, "seen".to_string()) {
		println!("`seen` table not found, creating...");
		if !sql_table_create(&conn, "seen".to_string()) {
			server.send_privmsg(&chan, "No seen table exists and for some reason I cannot create one");
			return;
		}
	}

	let count: i32 = conn.query_row("SELECT count(nick) FROM seen WHERE nick = ?", &[&who], |row| {
		row.get(0)
	}).unwrap();
	if count == 0 {
		let privmsg = format!("Sorry, I have not seen {}", who);
		server.send_privmsg(&chan, &privmsg);
		return;
	}

	result = conn.query_row("SELECT channel, said, datetime(ts, 'unixepoch'), nick, action FROM seen WHERE nick = ? COLLATE NOCASE ORDER BY ts DESC LIMIT 1", &[&who], |row| {
		SeenResult {
			channel: row.get(0),
			said: row.get(1),
			datetime: row.get(2),
			nick: row.get(3),
			action: match row.get(4) {
				1 => true,
				_ => false
			}
		}
	}).unwrap();
	
	if result.action {
		let privmsg = format!("[{}] {} *{} {}", result.datetime, result.channel, result.nick, result.said);
		server.send_privmsg(&chan, &privmsg);
	}
	else {
		let privmsg = format!("[{}] {} <{}> {}", result.datetime, result.channel, result.nick, result.said);
		server.send_privmsg(&chan, &privmsg);
	}
	return;
}

fn command_smake(server: &IrcServer, conn: &Connection, chan: &String, who: String) {
	if !sql_table_check(&conn, "smakes".to_string()) {
		println!("`smakes` table not found, creating...");
		if !sql_table_create(&conn, "smakes".to_string()) {
			server.send_privmsg(&chan, "No smakes table exists and for some reason I cannot create one");
			return;
		}
	}

	let check: i32 = conn.query_row("select count(*) from smakes", &[], |row| {
		row.get(0)
	}).unwrap();
	if check == 0 {
		server.send_privmsg(&chan, "No smakes in the database, add some.");
	}

	let result: String = conn.query_row("select smake from smakes order by random() limit 1", &[], |row| {
		row.get(0)
	}).unwrap();

	let dome: String = format!("smakes {} upside the head with {}", who, result);
	server.send_action(&chan, &dome);
}

fn command_smakeadd(server: &IrcServer, conn: &Connection, chan: &String, what: String) {
	if !sql_table_check(&conn, "smakes".to_string()) {
		println!("`smakes` table not found, creating...");
		if !sql_table_create(&conn, "smakes".to_string()) {
			server.send_privmsg(&chan, "No smakes table exists and for some reason I cannot create one");
			return;
		}
	}
	
	match conn.execute("INSERT INTO smakes VALUES(NULL, $1)", &[&what]) {
		Err(err) => {
			println!("{}", err);
			server.send_privmsg(&chan, "Error writing to smakes table.");
			return;
		},
		Ok(_) => {
			let sayme: String = format!("\"{}\" added.", what);
			server.send_privmsg(&chan, &sayme);
			return;
		},
	};
}

fn command_weatheradd(server: &IrcServer, conn: &Connection, nick: &String, chan: &String, checklocation: String) {
	
	if !sql_table_check(&conn, "locations".to_string()) {
		println!("locations table not found, creating...");
		if !sql_table_create(&conn, "locations".to_string()) {
			server.send_privmsg(&chan, "No locations table exists and for some reason I cannot create one");
			return;
		}
	}

	match conn.execute("REPLACE INTO locations VALUES($1, $2)", &[nick, &checklocation]) {
		Err(err) => {
			println!("{}", err);
			server.send_privmsg(&chan, "Error saving your location.");
		},
		Ok(_) => {
			let sayme: String = format!("Location for {} set to {}", nick, checklocation);
			server.send_privmsg(&chan, &sayme);
		},
	};
	return;
}

fn command_weather(botconfig: &BotConfig, server: &IrcServer, conn: &Connection, mut wucache: &mut Vec<CacheEntry>, nick: &String, chan: &String, checklocation: Option<String>) {
		let weather: String;
		let location: Option<String>;
		//is checklocation filled in? use it if it is.
		if checklocation.is_some() {
			let count: i32 = conn.query_row("SELECT count(nick) FROM locations WHERE nick = $1", &[&checklocation.clone().unwrap()], |row| {
					row.get(0)
			}).unwrap();
			if count == 1 {
				location = Some(conn.query_row("SELECT location FROM locations WHERE nick = $1", &[&checklocation.clone().unwrap()], |row| {
					row.get(0)
				}).unwrap());
			}
			else {
				location = checklocation;
			}
		}
		else {		
			let count: i32 = conn.query_row("SELECT count(location) FROM locations WHERE nick = $1", &[nick], |row| {
					row.get(0)
			}).unwrap();
			if count == 0 {
				location = None;
			}
			else {
				location = Some(conn.query_row("SELECT location FROM locations WHERE nick = $1", &[nick], |row| {
					row.get(0)
				}).unwrap());
			}
		}
		
		match location {
			Some(var) =>	weather = get_weather(&mut wucache, &botconfig.wu_key, var),
			None => weather = format!("No location found for {}", nick).to_string(),
		};

		server.send_privmsg(&chan, &weather);
		return;
}

fn command_abuser(server: &IrcServer, conn: &Connection, chan: &String, abuser: String) {
	if hostmask_add(&server, &conn, &chan, "abusers", &abuser) {
		let result: String = format!("Added '{}' to abusers.", &abuser);
		server.send_privmsg(&chan, &result);
	}
	else {
		let result: String = format!("Failed to add '{}' to abusers. Check the logs.", &abuser);
		server.send_privmsg(&chan, &result);
	}
	return;
}

fn command_admin(server: &IrcServer, conn: &Connection, chan: &String, admin: String) {
	if hostmask_add(&server, &conn, &chan, "admins", &admin) {
		let result: String = format!("Added '{}' to admins.", &admin);
		server.send_privmsg(&chan, &result);
	}
	else {
		let result: String = format!("Failed to add '{}' to admins. Check the logs.", &admin);
		server.send_privmsg(&chan, &result);
	}
	return;
}

fn command_bot(server: &IrcServer, conn: &Connection, chan: &String, bot: String) {
	if hostmask_add(&server, &conn, &chan, "bots", &bot) {
		let result: String = format!("Added '{}' to bots.", &bot);
		server.send_privmsg(&chan, &result);
	}
	else {
		let result: String = format!("Failed to add '{}' to bots. Check the logs.", &bot);
		server.send_privmsg(&chan, &result);
	}
	return;
}

fn command_help(server: &IrcServer, botconfig: &BotConfig, chan: &String, command: Option<String>) {
	let helptext: String = get_help(&botconfig.prefix, command);
	server.send_privmsg(&chan, &helptext);
}

fn sql_table_check(conn: &Connection, table: String) -> bool {
	let result: i32 = conn.query_row("SELECT count(name) FROM sqlite_master WHERE type = 'table' and name = ? LIMIT 1", &[&table], |row| {
			row.get(0)
	}).unwrap();
	
	if result == 1 {
		return true;
	}
	false
}

fn sql_table_create(conn: &Connection, table: String) -> bool {
	let schema: String = sql_get_schema(&table);
	match conn.execute(&schema, &[]) {
			Err(err) => { println!("{}", err); return false; },
			Ok(_) => { println!("{} table created", table); return true; },
	};
}

fn check_messages(conn: &Connection, nick: &String) -> bool {
	let table = "messages".to_string();
	if !sql_table_check(&conn, table.clone()) {
		return false;
	}
	let statement: String = format!("SELECT count(*) FROM {} WHERE recipient = $1", &table);
	let result: i32 = conn.query_row(statement.as_str(), &[&nick.as_str()], |row| {
			row.get(0)
	}).unwrap();
	if result > 0 {
		return true;
	}
	false
}

fn deliver_messages(server: &IrcServer, conn: &Connection, nick: &String) {
	struct Row {
		sender: String,
		//recipient: String,
		message: String,
		ts: i64
	};
	let mut timestamps: Vec<i64> = vec![];
	let mut stmt = conn.prepare(format!("SELECT * FROM messages WHERE recipient = '{}' ORDER BY ts", &nick).as_str()).unwrap();
	let mut allrows = stmt.query_map(&[], |row| {
		Row {
			sender: row.get(0),
			//recipient: row.get(1),
			message: row.get(2),
			ts: row.get(3)
		}
	}).unwrap();

	for row in allrows {
		let thisrow = row.unwrap();
		server.send_privmsg(&nick, format!("<{}> {}", thisrow.sender, thisrow.message).as_str());
		timestamps.push(thisrow.ts);
	}
	
	for ts in timestamps.iter() {
		let statement = format!("DELETE FROM messages WHERE recipient = '{}' AND ts = {}", &nick, &ts);
		match conn.execute(statement.as_str(), &[]) {
			Err(err) => println!("{}", err),
			Ok(_) => {}
		};
	}
	return;
}

fn save_msg(conn: &Connection, fromwho: &String, tellwho: String, tellwhat: String) -> bool {
	let table = "messages".to_string();
	if !sql_table_check(&conn, table.clone()) {
		if !sql_table_create(&conn, table.clone()) {
			return false;
		}
	}
	
	let time: i64 = time::now_utc().to_timespec().sec;
	let statement: String = format!("INSERT INTO {} VALUES($1, $2, $3, $4)", &table).to_string();
	match conn.execute(statement.as_str(), &[&fromwho.as_str(), &tellwho, &tellwhat, &time]) {
		Err(err) => {
			println!("{}", err);
			return false;
		},
		Ok(_) => return true,
	};
}

fn get_help(prefix: &String, command: Option<String>) -> String {
	if command.is_none() {
		return "Commands: help, weatheradd, weather, submit, seen, smake, smakeadd, youtube, abuser, bot, admin, socialist, roll, bnk, join, part, tell, klingon, g".to_string();
	}
	let inside = command.unwrap();
	match &inside[..] {
		"help" => format!("Yes, recursion is nifty. Now piss off."),
		"weatheradd" => format!("{}weatheradd <zip> or {}weatheradd city, st", prefix, prefix),
		"weather" => format!("{}weather <zip>, {}weather city, st, or just {}weather if you have saved a location with {}weatheradd", prefix, prefix, prefix, prefix),
		"submit" => format!("{}submit <url> or {}submit <url> <what you have to say about it>", prefix, prefix),
		"seen" => format!("{}seen <nick>", prefix),
		"smake" => format!("{}smake <someone>", prefix),
		"smakeadd" => format!("{}smakeadd <something to smake people with> e.g. {}smakeadd a half-brick in a sock", prefix, prefix),
		"abuser" => format!("limits the commands a jackass can use. {}abuser <full @hostmask> e.g. {}abuser @Soylent/Staff/Editor/cmn32480", prefix, prefix),
		"bot" => format!("registers a hostmask as a bot. {}bot <full @hostmask> e.g. {}bot @universe2.us/ircbot/aqu4", prefix, prefix),
		"admin" => format!("give someone godlike powers over this bot. {}admin <full @hostmask> e.g. {}admin @Soylent/Staff/Developer/TMB", prefix, prefix),
		"youtube" => format!("search youtube. {}youtube <search string>", prefix),
		"socialist" => format!("has half of a libertarian debate. {}socialist <nick>", prefix),
		"roll" => format!("roll the dice. {}roll 3d6 throws=6", prefix),
		"bnk" => format!("BOOBIES n KITTEHS!"),
		"join" => format!("{}join <channel>", prefix),
		"part" => format!("{}part or {}part <channel>", prefix, prefix),
		"tell" => format!("{}tell <nick> <message>", prefix),
		"klingon" => format!("translate something to klingon. {}klingon <phrase>", prefix),
		"g" => format!("google search. {}g <search query>", prefix),
		_ => format!("{}{} is not a currently implemented command", prefix, inside),
	}
}

fn sql_get_schema(table: &String) -> String {
	match &table[..] {
		"seen" => "CREATE TABLE seen(nick TEXT, hostmask TEXT, channel TEXT, said TEXT, ts UNSIGNED INT(8), action UNSIGNED INT(1) CHECK(action IN(0,1)), primary key(nick, channel) )".to_string(),
		"smakes" => "CREATE TABLE smakes (id INTEGER PRIMARY KEY AUTOINCREMENT, smake TEXT NOT NULL)".to_string(),
		"sammiches" => "CREATE TABLE sammiches (id INTEGER PRIMARY KEY AUTOINCREMENT, sammich TEXT NOT NULL)".to_string(),
		"bot_config" => "CREATE TABLE bot_config(nick TEXT PRIMARY KEY, server TEXT, channel TEXT, perl_file TEXT, prefix TEXT, admin_hostmask TEXT, snpass TEXT, snuser TEXT, cookiefile TEXT, wu_api_key TEXT, google_key text)".to_string(),
		"locations" => "CREATE TABLE locations(nick TEXT PRIMARY KEY, location TEXT)".to_string(),
		"bots" => "CREATE TABLE bots(hostmask TEXT PRIMARY KEY NOT NULL)".to_string(),
		"abusers" => "CREATE TABLE abusers(hostmask TEXT PRIMARY KEY NOT NULL)".to_string(),
		"admins" => "CREATE TABLE admins(hostmask PRIMARY KEY NOT NULL)".to_string(),
		"test" => "CREATE TABLE test(hostmask PRIMARY KEY NOT NULL)".to_string(),
		"messages" => "CREATE TABLE messages(sender TEXT, recipient TEXT, message TEXT, ts UNSIGNED INT(8))".to_string(),
		"feeds" => "CREATE TABLE feeds(id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, address TEXT NOT NULL, frequency INTEGER, lastchecked TEXT)".to_string(),
		"feed_items" => "CREATE TABLE feed_items(feed_id INTEGER, md5sum TEXT, PRIMARY KEY (feed_id, md5sum))".to_string(),
		_ => "".to_string(),
	}
}

fn cache_push(mut cache: &mut Vec<CacheEntry>, location: &String, weather: &String) {
	cache_prune(&mut cache);
	let entry = CacheEntry {
		age: time::now_utc().to_timespec().sec,
		location: location.to_string().clone(),
		weather: weather.to_string().clone(),
	};
	cache.push(entry);
	return;
}

fn cache_dump(cache: Vec<CacheEntry>) {
	println!("{:?}", cache);
}

fn cache_get(mut cache: &mut Vec<CacheEntry>, location: &String) -> Option<String> {
	cache_prune(&mut cache);
	let position: Option<usize> = cache.iter().position(|ref x| x.location == location.to_string());
	if position.is_some() {
		let weather: &String = &cache[position.unwrap()].weather;
		return Some(weather.to_string().clone());
	}
	None
}

fn cache_prune(mut cache: &mut Vec<CacheEntry>) {
	if cache.is_empty() { return; }
	let oldest: i64 = time::now_utc().to_timespec().sec - 14400;
	loop {
		let position = cache.iter().position(|ref x| x.age < oldest);
		match position {
			Some(var) => cache.swap_remove(var),
			None => break,
		};
	}
	cache.shrink_to_fit();
}

fn get_weather(mut wucache: &mut Vec<CacheEntry>, wu_key: &String, location: String) -> String {
	let cached = cache_get(&mut wucache, &location);
	if cached.is_some() {
		return cached.unwrap();
	}

	let mut dst = Vec::new();
	{
		let mut callback = |data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		};
		let mut easy = Easy::new();
		let encloc = fix_location(&location).to_string();
		//let querybytes = locstr.clone().into_bytes();
		//let encloc = easy.url_encode(&querybytes[..]);
		let url = format!("http://api.wunderground.com/api/{}/forecast/q/{}.json", wu_key.to_string(), encloc.to_string());
		easy.url(url.as_str()).unwrap();
		easy.write_function(&mut callback).unwrap();
		easy.perform().unwrap();
	
		if easy.response_code().unwrap_or(999) != 200 {
			return format!("got http response code {}", easy.response_code().unwrap_or(999)).to_string();
		}
	}


	let json = str::from_utf8(&dst[..]).unwrap();
	//return page.to_string().trim().to_string();

	/*let resp = http::handle().get(url).exec().unwrap();
	if resp.get_code() != 200 {
		return format!("got http response code {}", resp.get_code()).to_string();
	}
	let json = str::from_utf8(resp.get_body()).unwrap();*/

	let jsonthing = Json::from_str(json).unwrap();
	let forecast;
	if jsonthing.find_path(&["forecast", "txt_forecast", "forecastday" ]).is_some() {
		forecast = jsonthing.find_path(&["forecast", "txt_forecast", "forecastday" ]).unwrap();
	}
	else {return "Unable to find weather for that location".to_string();}
	let today = forecast[0].find_path(&["fcttext"]).unwrap().as_string().unwrap();
	let tomorrow = forecast[2].find_path(&["fcttext"]).unwrap().as_string().unwrap();
	let forecast = format!("Today: {} Tomorrow: {}", today, tomorrow);
	cache_push(&mut wucache, &location, &forecast);
	return forecast;
}

fn fix_location(location: &String) -> String {
	let numeric = location.parse::<u32>();
	
	if numeric.is_ok() {
		let zip: String = numeric.unwrap().to_string();
		return zip;
	}

	let comma = location.find(',').unwrap_or(location.len());
	if comma < location.len() {
		let location = location.clone();
		let (city, state) = location.split_at(comma);
		let mut easy = Easy::new();
		let citybytes = city.clone().to_string().into_bytes();
		let statebytes = state.clone().to_string().into_bytes();
		let enccity = easy.url_encode(&citybytes[..]);
		let encstate = easy.url_encode(&statebytes[..]);
		let citystate = format!("{}/{}", encstate.trim_left_matches(",").trim(), enccity.trim()).to_string();
		return citystate;
	}
	else {
		let failed = "dohelp".to_string();
		return failed;
	}
}

fn hostmask_add(server: &IrcServer, conn: &Connection, chan: &String, table: &str, hostmask: &String) -> bool {
	if !sql_table_check(&conn, table.to_string()) {
		println!("{} table not found, creating...", table);
		if !sql_table_create(&conn, table.to_string()) {
			let err: String = format!("No {} table exists and for some reason I cannot create one. Check the logs.", table);
			server.send_privmsg(&chan, &err);
			return false;
		}
	}
	
	let statement: String = format!("INSERT INTO {} VALUES($1)", &table).to_string();
	match conn.execute(statement.as_str(), &[&hostmask.as_str()]) {
		Err(err) => {
			println!("{}", err);
			return false;
		},
		Ok(_) => return true,
	};
}

fn is_admin(botconfig: &BotConfig, server: &IrcServer, conn: &Connection, chan: &String, hostmask: &String) -> bool {
	let table = "admins".to_string();
	if !sql_table_check(&conn, table.clone()) {
		println!("{} table not found, creating...", &table);
		if !sql_table_create(&conn, table.clone()) {
			let err: String = format!("No {} table exists and for some reason I cannot create one. Check the logs.", &table);
			server.send_privmsg(&chan, &err);
			return false;
		}
		let statement: String = format!("INSERT INTO {} VALUES($1)", &table).to_string();
		match conn.execute(statement.as_str(), &[&botconfig.admin.as_str()]) {
			Err(err) => {
				println!("{}", err);
				return false;
			},
			Ok(_) => {},
		};
	}
	
	let statement: String = format!("SELECT count(*) FROM {} WHERE hostmask = $1", &table);
	let hostmask: String = hostmask_only(&hostmask);
	let result: i32 = conn.query_row(statement.as_str(), &[&hostmask], |row| {
			row.get(0)
	}).unwrap();
	if result == 1 {
		return true;
	}
	false
}

fn is_bot(server: &IrcServer, conn: &Connection, chan: &String, hostmask: &String) -> bool {
	let table = "bots".to_string();
	if !sql_table_check(&conn, table.clone()) {
		println!("{} table not found, creating...", &table);
		if !sql_table_create(&conn, table.clone()) {
			let err: String = format!("No {} table exists and for some reason I cannot create one. Check the logs.", &table);
			server.send_privmsg(&chan, &err);
			return false;
		}
	}
	
	let statement: String = format!("SELECT count(*) FROM {} WHERE hostmask = $1", &table);
	let hostmask: String = hostmask_only(&hostmask);
	let result: i32 = conn.query_row(statement.as_str(), &[&hostmask], |row| {
			row.get(0)
	}).unwrap();
	if result == 1 {
		return true;
	}
	false
}

fn is_abuser(server: &IrcServer, conn: &Connection, chan: &String, hostmask: &String) -> bool {
	let table = "abusers".to_string();
	if !sql_table_check(&conn, table.clone()) {
		println!("{} table not found, creating...", &table);
		if !sql_table_create(&conn, table.clone()) {
			let err: String = format!("No {} table exists and for some reason I cannot create one. Check the logs.", &table);
			server.send_privmsg(&chan, &err);
			return false;
		}
	}
	
	let statement: String = format!("SELECT count(*) FROM {} WHERE hostmask = $1", &table);
	let hostmask: String = hostmask_only(&hostmask);
	let result: i32 = conn.query_row(statement.as_str(), &[&hostmask], |row| {
			row.get(0)
	}).unwrap();
	if result == 1 {
		return true;
	}
	false
}

fn hostmask_only(fullstring: &String) -> String {
	let position: Option<usize> = fullstring.as_str().find("@");
	if position.is_some() {
		let here = position.unwrap();
		let maskonly = fullstring[here..].to_string();
		return maskonly;
	}
	"OMGWTFBBQ".to_string()
}

fn sub_parse_line(noprefix: &String) -> (String, String) {
	let http: Option<usize> = noprefix[..].to_string().trim().to_string().find("http");
	let mut suburl: String = " ".to_string();
	let mut summary: String = " ".to_string();
	if http.is_some() {
		let beginurl = http.unwrap();
		let preurl = noprefix[beginurl..].to_string().trim().to_string();
		let space: Option<usize> = preurl.find(" ");
		if space.is_some() {
			let sp = space.unwrap();
			suburl = preurl[..sp].to_string().trim().to_string();
			summary = preurl[sp..].to_string().trim().to_string();
		}
		else {
			suburl = preurl;
		}
	}
	else {
		println!("http not found");
	}
	(suburl, summary)
}

fn sub_get_page(url: &String) -> String {
	let mut dst = Vec::new();
	{
		let mut callback = |data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		};
		let mut easy = Easy::new();
		easy.url(url.as_str()).unwrap();
		easy.write_function(&mut callback).unwrap();
		easy.perform().unwrap();
	
		if easy.response_code().unwrap_or(999) != 200 {
			return format!("got http response code {}", easy.response_code().unwrap_or(999)).to_string();
		}
	}

	let page = str::from_utf8(&dst[..]).unwrap();
	return page.to_string().trim().to_string();
	/*let resp = http::handle().get(url).exec().unwrap();
	if resp.get_code() != 200 {
		return format!("got http response code {}", resp.get_code()).to_string();
	}
	let page = str::from_utf8(resp.get_body()).unwrap();
	return page.to_string().trim().to_string();*/
}

fn sub_get_title(titleres: &Vec<Regex>, page: &String) -> String {
	let mut title = "".to_string();
	for regex in titleres.iter() {
		let captures = regex.captures(page.as_str());
		if captures.is_none() {
			title = "".to_string();
		}
		else {
			let cap = captures.unwrap().at(1).unwrap();
			title = cap.to_string().trim().to_string();
			break;
		}
	}
	return title;
}

fn sub_get_description(descres: &Vec<Regex>, page: &String) -> String {
	let mut desc = "".to_string();
	for regex in descres.iter() {
		let captures = regex.captures(page.as_str());
		if captures.is_none() {
			desc = "".to_string();
		}
		else {
			let unwrapped = captures.unwrap();
			let cap = unwrapped.at(1).unwrap();
			desc = cap.to_string().trim().to_string();
			break;
		}
	}	
	return desc;
}

//let story = sub_build_story( &submitter, &description, &summary )
fn sub_build_story(submitter: &String, description: &String, summary: &String, source: &String) -> String {
	let story = format!("Submitted via IRC for {}<blockquote>{}</blockquote>{}\n\nSource: {}", submitter, description, summary, source).to_string();
	return story;
}

fn sub_get_reskey(cookie: &String) -> String {
	let url: String;
	if DEBUG {
		url = "https://dev.soylentnews.org/api.pl?m=story&op=reskey".to_string().trim().to_string();
	}
	else {
		url = "https://soylentnews.org/api.pl?m=story&op=reskey".to_string().trim().to_string();
	}

	let mut dst = Vec::new();
	{
		let mut callback = |data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		};
		let mut easy = Easy::new();
		easy.url(url.as_str()).unwrap();
		easy.cookie(cookie.as_str()).unwrap();
		easy.write_function(&mut callback).unwrap();
		easy.perform().unwrap();
	
		if easy.response_code().unwrap_or(999) != 200 {
			println!("got http response code {}", easy.response_code().unwrap_or(999));
			return "".to_string();
		}
	}
	
	let unparsed = str::from_utf8(&dst[..]).unwrap();
	let jsonthing = Json::from_str(unparsed).unwrap();
	let resopt = jsonthing.find("reskey");
	let mut reskey: String;
	if resopt.is_some() {
		reskey = resopt.unwrap().to_string().trim().to_string();
		let creskey = reskey.clone();
		let strreskey = creskey.as_str();
		let len = strreskey.len() - 1;
		reskey = strreskey[1..len].to_string().trim().to_string();
	}
	else {
		reskey = "".to_string();
	}
	return reskey;
}

fn sub_get_cookie(botconfig: &mut BotConfig) -> String {
	if botconfig.cookie != "".to_string() {
		return botconfig.cookie.clone();
	}
	let url: String;
	if DEBUG {
		url = format!("https://dev.soylentnews.org/api.pl?m=auth&op=login&nick={}&pass={}", "MrPlow", &botconfig.snpass).to_string();
	}
	else {
		url = format!("https://soylentnews.org/api.pl?m=auth&op=login&nick={}&pass={}", &botconfig.nick, &botconfig.snpass).to_string();
	}
	let mut dst = Vec::new();
	{
		let mut callback = |data: &[u8]| {
			dst.extend_from_slice(data);
			true
		};
		let mut nullme = |data: &[u8]| {
			data.len()
		};
		let mut easy = Easy::new();
		easy.url(url.as_str()).unwrap();
		easy.header_function(&mut callback).unwrap();
		easy.write_function(&mut nullme).unwrap();
		easy.perform().unwrap();
	
		if easy.response_code().unwrap_or(999) != 200 {
			println!("got http response code {}", easy.response_code().unwrap_or(999));
			return "".to_string();
		}
	}

	let headers = str::from_utf8(&dst[..]).unwrap().split("\n");
	let mut cookie: String = "".to_string();
	for foo in headers {
		if foo.find("Set-Cookie:").unwrap_or(22) != 22 {
			cookie = foo[12..].to_string().trim().to_string();
			let ccookie = cookie.clone();
			let strcookie = ccookie.as_str();
			let end = strcookie.find("path=/;").unwrap_or(22_usize) + 7;
			cookie = strcookie[..end].to_string().trim().to_string();
		}
	}
	if cookie != "".to_string() {
		botconfig.cookie = cookie.clone();
	}
	return cookie;
}

fn load_titleres(exists: Option<Vec<Regex>>) -> Vec<Regex> {
	let titleresf = OpenOptions::new().read(true).write(true).create(true).open("/home/bob/etc/snbot/titleres.txt");
	if titleresf.is_err() {
		println!("Error opening titleres.txt: {:?}", titleresf);
		let wtf: Vec<Regex> = Vec::new();
		return wtf;
	}
	let unwrapped = &titleresf.unwrap();
	let titleresfile = BufReader::new(unwrapped);

	match exists {
		None => {
			let mut titleres: Vec<Regex> = Vec::new();
			for line in titleresfile.lines() {
				if line.is_ok() {
					titleres.push(Regex::new(line.unwrap().as_str()).unwrap());
				}
			}
			return titleres;
		},
		Some(mut titleres) => {
			titleres.clear();
			for line in titleresfile.lines() {
				if line.is_ok() {
					titleres.push(Regex::new(line.unwrap().as_str()).unwrap());
				}
			}
			return titleres;
		},
	};
}

fn load_descres(exists: Option<Vec<Regex>>) -> Vec<Regex> {
	let descresf = OpenOptions::new().read(true).write(true).create(true).open("/home/bob/etc/snbot/descres.txt");
	if descresf.is_err() {
		println!("Error opening descres.txt: {:?}", descresf);
		let wtf: Vec<Regex> = Vec::new();
		return wtf;
	}
	let damnit = &descresf.unwrap();
	let descresfile = BufReader::new(damnit);

	match exists {
		None => {
			let mut descres: Vec<Regex> = Vec::new();
			for line in descresfile.lines() {
				if line.is_ok() {
					descres.push(Regex::new(line.unwrap().as_str()).unwrap());
				}
			}
			return descres;
		},
		Some(mut descres) => {
			descres.clear();
			for line in descresfile.lines() {
				if line.is_ok() {
					descres.push(Regex::new(line.unwrap().as_str()).unwrap());
				}
			}
			return descres;
		},
	};
}

fn get_youtube(go_key: &String, query: &String) -> String {
	let mut dst = Vec::new();
	{
		let mut easy = Easy::new();
		let querybytes = query.clone().into_bytes();
		let encquery = easy.url_encode(&querybytes[..]);
		//let querystr = String::from_utf8(encquery).unwrap();
		let url = format!("https://www.googleapis.com/youtube/v3/search/?maxResults=1&q={}&order=relevance&type=video&part=snippet&key={}", encquery, go_key);
		easy.url(url.as_str()).unwrap();
		easy.write_function(|data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		});
		easy.fail_on_error(true);
		easy.perform().unwrap();

		if easy.response_code().unwrap_or(999) != 200 {
			println!("got http response code {}", easy.response_code().unwrap_or(999));
			return "Something borked, check the logs.".to_string();
		}
	}
	let json = str::from_utf8(&dst[..]).unwrap();
	let jsonthing = Json::from_str(json).unwrap();
	let resopt = jsonthing.find_path(&["items"]).unwrap();
	let cleanme = resopt[0].find_path(&["id", "videoId"]).unwrap().as_string().unwrap().to_string();
	return cleanme;

}

fn send_submission(submission: &Submission) -> bool {
	let url: String;
	if DEBUG {
		url = "https://dev.soylentnews.org/api.pl?m=story&op=post".to_string().trim().to_string();
	}
	else {
		url = "https://soylentnews.org/api.pl?m=story&op=post".to_string().trim().to_string();
	}
	let fooclone;
	let mut postdata = "foo".as_bytes();
	let mut dst = Vec::new();
	{
		let mut easy = Easy::new();
		let subjectbytes = submission.subject.clone().into_bytes();
		let encsubject = easy.url_encode(&subjectbytes[..]);
		let storybytes = submission.story.clone().into_bytes();
		let encstory = easy.url_encode(&storybytes[..]);
		if DEBUG {
			let foo = format!("primaryskid=1&sub_type=plain&tid=10&name=MrPlow&reskey={}&subj={}&story={}", submission.reskey, encsubject, encstory);
			println!("{}", foo);
			fooclone = foo.clone();
			postdata = fooclone.as_bytes();
		}
		else {
			let foo = format!("primaryskid=1&sub_type=plain&tid=10&name={}&reskey={}&subj={}&story={}", submission.botnick, submission.reskey, encsubject, encstory);
			fooclone = foo.clone();
			postdata = fooclone.as_bytes();
		}

		easy.url(url.as_str()).unwrap();
		easy.cookie(submission.cookie.as_str()).unwrap();
		easy.write_function(|data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		}).unwrap();
		easy.post_field_size(postdata.len() as u64).unwrap();
		easy.post_fields_copy(postdata).unwrap();
		easy.post(true).unwrap();
		easy.fail_on_error(true);
		easy.perform().unwrap();
	
		if easy.response_code().unwrap_or(999) != 200 {
			println!("got http response code {} for send_submission", easy.response_code().unwrap_or(999));
			return false;
		}
	}
	let output: String = String::from_utf8(dst).unwrap_or("".to_string());
	println!("{}", output);
	return true;
}

fn get_bing_token(botconfig: &BotConfig) -> String {
	let url = "https://datamarket.accesscontrol.windows.net/v2/OAuth2-13/";
	let mut postdata = "foo".as_bytes();
	let mut dst = Vec::new();
	{
		let mut easy = Easy::new();
		let secretbytes = &botconfig.bi_key[..].as_bytes();
		let postfields = format!("grant_type=client_credentials&scope=http://api.microsofttranslator.com&client_id=TMBuzzard_Translator&client_secret={}", easy.url_encode(secretbytes));
		let postbytes = postfields.as_bytes();
		easy.url(url).unwrap();
		easy.write_function(|data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		});
		easy.post_field_size(postbytes.len() as u64).unwrap();
		easy.post_fields_copy(postbytes).unwrap();
		easy.post(true).unwrap();
		easy.perform().unwrap();

		if easy.response_code().unwrap_or(999) != 200 {
			println!("got http response code {} for get_bing_token", easy.response_code().unwrap_or(999));
			return "".to_string();
		}
	}
	let json: String = String::from_utf8(dst).unwrap_or("".to_string());
	let jsonthing = Json::from_str(json.as_str()).unwrap();
	let tokenopt = jsonthing.find("access_token");
	let mut token: String;
	if tokenopt.is_some() {
		token = tokenopt.unwrap().to_string().trim().to_string();
		if &token[0..1] == "\"" {
			let ctoken = token.clone();
			let strtoken = ctoken.as_str();
			let len = strtoken.len() - 1;
			token = strtoken[1..len].to_string().trim().to_string();
		}
	}
	else {
		token = "".to_string();
	}
	return token;
}

fn is_ns_faker(server: &IrcServer, nick: &String) -> bool {
	return false;
}

fn is_nick_here(server: &IrcServer, chan: &String, nick: &String) -> bool {
	let nicklist = server.list_users(&chan.as_str());
	if nicklist.is_none() {
		println!("got NONE for server.list_users('{}')", &chan);
		return false;
	}
	for user in nicklist.unwrap() {
		if &user.get_nickname() == &nick.as_str() {
			return true;
		}
	}
	return false;
}

fn get_raw_feed(feed: &String) -> String {
	let mut dst = Vec::new();
	{
		let mut easy = Easy::new();
		let url = feed.clone();
		easy.url(url.as_str()).unwrap();
		easy.write_function(|data: &[u8]| {
			dst.extend_from_slice(data);
			data.len()
		});
		easy.fail_on_error(true);
		easy.perform().unwrap();

		if easy.response_code().unwrap_or(999) != 200 {
			println!("got http response code {}", easy.response_code().unwrap_or(999));
			return "Something borked, check the logs.".to_string();
		}
	}
	let feed_data = str::from_utf8(&dst[..]).unwrap();
	return feed_data.to_string();
}

fn get_feed_title(feed: String) -> String {
	let feedstr = feed.as_str();
	if is_atom(&feedstr) {
		let parsed = feedstr.parse::<Feed>().unwrap();
		return parsed.title.to_string();
	}
	else if is_rss2(&feedstr) {
		let parsed = feedstr.parse::<Rss>().unwrap();
		return parsed.0.title.to_string();
	}
	else {
		return "Unknown feed type".to_string();
	}
	//let parsed = feedstr.parse::<Feed>().unwrap();
	//println!("{:?}", parsed.title);
	//return parsed.Channel.title.to_string();
	//return "foo".to_string();
}

fn is_atom(feedstr: &str) -> bool {
	let re = Regex::new(r"xmlns=\S+Atom").unwrap();
	if re.is_match(feedstr) {
		return true;
	}
	else {
		return false;
	}
}

fn is_rss2(feedstr: &str) -> bool {
	let re = Regex::new(r"<rss.*?version=.2\.0.").unwrap();
	if re.is_match(feedstr) {
		return true;
	}
	else {
		return false;
	}
}

// Begin DnD code
fn dnd_control(conn: &Connection, msg: &String) {
	let sekrit = "jigglyboobs".to_string(); // This needs to come from botconfig
	let reg = Regex::new(r"^(\w),(\d+),(\d+),(\d+) (.*)$").unwrap();
	let cap = reg.captures(&msg.as_str());
	if cap.is_none() { return; }
	let captures = cap.unwrap();
	let cmd = captures.at(1).unwrap_or("");
	let uid: i64;
	if captures.at(2).is_some() {
		uid = captures.at(2).unwrap().parse().unwrap();
	}
	else { uid = 0_i64; }
	let itemid: i64;
	if captures.at(3).is_some() {
		itemid = captures.at(3).unwrap().parse().unwrap();
	}
	else { itemid = 0_i64; }
	let count: i64;
	if captures.at(4).is_some() {
		count = captures.at(4).unwrap().parse().unwrap();
	}
	else { count = 0_i64; }
	let sig = captures.at(5).unwrap_or("");
	if cmd == "g," {
		if check_signature(&sekrit, &msg) {
			item_receive(&conn, uid, itemid, count);
		}
		else { return; }
	}
	else if cmd == "t," {
		if check_signature(&sekrit, &msg) {
			item_remove(&conn, uid, itemid, count);
		}
	}
	else { return; }
	return;
}

fn item_receive(conn: &Connection, uid: i64, itemid: i64, count: i64) {
	let mut has: i64 = conn.query_row("SELECT count FROM player_inventory WHERE uid = $1 AND itemid = $2", &[&uid, &itemid], |row| {
		row.get(0)
	}).unwrap_or(0_i64);
	if has != 0 {
		has += count;
		conn.execute("UPDATE player_inventory SET count = $1 WHERE uid = $2 AND itemid = $3", &[&has, &uid, &itemid]).unwrap();
	}
	else {
		let slot = 0_i64;
		conn.execute("INSERT INTO player_inventory VALUES($1, $2, $3, $4)", &[&uid, &itemid, &slot, &count]).unwrap();
	}
	return;
}

fn item_remove(conn: &Connection, uid: i64, itemid: i64, count: i64) {
	let mut has: i64 = conn.query_row("SELECT count FROM player_inventory WHERE uid = $1 AND itemid = $2", &[&uid, &itemid], |row| {
		row.get(0)
	}).unwrap_or(0_i64);
	let newval = count - has;
	if newval >= 0 || itemid == ITEM_MONEY {
		conn.execute("UPDATE player_inventory SET count = $1 WHERE uid = $2 AND itemid = $3", &[&newval, &uid, &itemid]).unwrap();
	}
	else {
		// say back to whatever channel this came in on that only money can go negative
	}
	return;
}

fn mv_internal() {
}

fn send_inventory() {
}

//sekrit = "geezerboobs"; msg = "g/t,uid,itemid,count"; sig = openssl_digest(msg . sekrit, 'sha512'); signedmsg = msg . " " . sig;
fn get_external_cmd() {
}

fn check_signature(sekrit: &String, msg: &String) -> bool {
	let mut space = msg.find(" ").unwrap_or(0);
	let unsigned = &msg[..space];
	space += 1;
	let checkme = format!("{}{}", sekrit, unsigned);
	let checksig = msg[space..].to_string();
	let mut hasher = Sha512::new();
	hasher.input_str(checkme.as_str());
	let sig = hasher.result_str().to_string();
	if checksig == sig {
		return true;
	}
	
	false
}

fn sign_msg(sekrit: &String, msg: &String) -> String {
	let mut hasher = Sha512::new();
	let hashme = format!("{}{}", sekrit, msg);
	hasher.input_str(hashme.as_str());
	let sig = hasher.result_str();
	let signedmsg = format!("{} {}", msg, sig);
	return signedmsg;
}



/*fn command_sign(server: &IrcServer, chan: &String, msg: String) {
	let sekrit = "jigglyboobs".to_string();
	let signed = sign_msg(&sekrit, &msg);
	server.send_privmsg(&chan, &signed);
}*/

fn roll_stats(conn: &Connection, nick: &String) -> Vec<i64> {
	let mut stats = Vec::new();
	let mut rng = rand::thread_rng();
	for _ in 1..7 {
		let mut rolls = Vec::new();
		for _ in 1..5 {
			let random = rng.gen::<u64>();
			let roll: i64 = ((random % 6) + 1) as i64;
			rolls.push(roll);
		}
		rolls.sort();
		stats.push(rolls[1] + rolls[2] + rolls[3]);
	}
	if character_exists(&conn, &nick) {
		let stmt = "UPDATE players SET race = NULL, class = NULL, level = 1, strength = $1, constitution = $2, dexterity = $3, intelligence = $4, wisdom = $5, charisma = $6, abilities = 0 WHERE nick = $7";
		conn.execute(stmt, &[&stats[0], &stats[1], &stats[2], &stats[3], &stats[4], &stats[5], &nick.as_str()]).unwrap();
	}
	else {
		let stmt = "INSERT INTO players (nick, race, class, level, strength, constitution, dexterity, intelligence, wisdom, charisma, abilities) VALUES( $1, NULL, NULL, 1, $2, $3, $4, $5, $6, $7, 0)";
		conn.execute(stmt, &[&nick.as_str(), &stats[0], &stats[1], &stats[2], &stats[3], &stats[4], &stats[5]]).unwrap();
	}
	return stats;
}

fn character_exists(conn: &Connection, nick: &String) -> bool {
	let count: i32 = conn.query_row("SELECT count(*) FROM players WHERE nick = $1", &[&nick.as_str()], |row| {
		row.get(0)
	}).unwrap();
	if count == 0 { return false; }
	return true;
}

fn set_race(conn: &Connection, nick: &String, race: &String) {
	conn.execute("UPDATE players SET race = $1 WHERE nick = $2", &[&race.as_str(), &nick.as_str()]).unwrap();
	let (pstr, pcon, pdex, pint, pwis, pcha) = get_racial_stat_adjustments(&race);
	let abilities = get_racial_abilities(&race);
	let stmt = format!("UPDATE players SET strength = strength + $1, constitution = constitution + $2, dexterity = dexterity + $3, intelligence = intelligence + $4, wisdom = wisdom + $5, charisma = charisma + $6, abilities = {} WHERE nick = $7", abilities);
	conn.execute(stmt.as_str(), &[&pstr, &pcon, &pdex, &pint, &pwis, &pcha, &nick.as_str()]).unwrap();
	return;
}

fn set_class(conn: &Connection, nick: &String, class: &String) {
	conn.execute("UPDATE players SET class = $1 WHERE nick = $2", &[&class.as_str(), &nick.as_str()]).unwrap();
	return;
}

fn list_races() -> &'static [&'static str] {
	return &RACES;
}

fn list_classes() -> &'static [&'static str] {
	return &CLASSES;
}

fn is_valid_race(race: &String) -> bool {
	let races = list_races();
	if races.contains(&race.as_str()) {
		return true;
	}
	return false;
}

fn is_valid_class(class: &String) -> bool {
	let classes = list_classes();
	if classes.contains(&class.as_str()) {
		return true;
	}
	return false;
}

fn display_player_info(conn: &Connection, player: &String, full: bool) -> String {
	if !is_race_set(&conn, &player) || !is_class_set(&conn, &player) {
		return format!("The character {} has not yet completed set-up.", &player);
	}
	let info: String;
	if full {
		let (uid, race, class, level, strength, con, dex, int, wis, cha, abilitiesi) =
		conn.query_row("SELECT uid, race, class, level, strength, constitution, dexterity, intelligence, wisdom, charisma, abilities FROM players WHERE nick = $1", &[&player.as_str()], |row| {
			( row.get(0), row.get(1), row.get(2), row.get(3), row.get(4), row.get(5), row.get(6), row.get(7), row.get(8), row.get(9), row.get(10) )
		}).unwrap_or( (0_i32, "".to_string(), "".to_string(), 0_i64, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i64) );
		let abilities = abilitiesi as u64;
		let abstr: String = read_abilities(abilities);
		info = format!("{}({}) {} {}({}) Str: {} Con: {} Dex: {} Int: {} Wis: {} Cha: {} Abilities: {}", &player, uid, race, class, level, strength, con, dex, int, wis, cha, abstr);
	}
	else {
		let (uid, race, class, level) = conn.query_row("SELECT uid, race, class, level FROM players WHERE nick = $1", &[&player.as_str()], |row| {
			(row.get(0), row.get(1), row.get(2), row.get(3))
		}).unwrap_or( (0_i64, "".to_string(), "".to_string(), 0_i64) );
		info = format!("{}({}): {} {}({})", &player, uid, race, class, level);
	}
	return info;
}

fn get_ability_description(ability: u64) -> String {
	let description = match ability {
		ABILITY_DARKVISION		=> "darkvision",
		ABILITY_SUPDARKVISION		=> "superior darkvision",
		ABILITY_DWFRESILIENCE		=> "dwarven resilience",
		ABILITY_DWFCOMBATTRAIN		=> "dwarven combat training",
		ABILITY_DWFTOOLPROF		=> "dwarven tool proficiency",
		ABILITY_DWFTOUGHNESS		=> "dwarven toughness",
		ABILITY_DWFARMTRAIN		=> "dwarven armor training",
		ABILITY_ELFKEENSENS		=> "keen senses",
		ABILITY_ELFFEYANCEST		=> "fey ancestry",
		ABILITY_ELFCOMBATTRAIN		=> "elf weapon training",
		ABILITY_ELFCANTRIP		=> "high elf cantrip",
		ABILITY_ELFMOTW			=> "mark of the wild",
		ABILITY_DELFSUNBAD		=> "sunlight sensitivity",
		ABILITY_DELFMAGIC		=> "drow magic",
		ABILITY_DELFWEAPON		=> "drow weapon training",
		ABILITY_HFLLUCK			=> "lucky",
		ABILITY_HFLBRAVE		=> "brave",
		ABILITY_HFLSTEALTH		=> "naturally stealthy",
		ABILITY_HFLRESILIENCE		=> "stout resilience",
		ABILITY_DBNFIRE			=> "fire breath weapon and resistance",
		ABILITY_DBNCOLD			=> "cold breath weapon and resistance",
		ABILITY_DBNACID			=> "acid breath weapon and resistance",
		ABILITY_DBNLIGHT		=> "lightning breath weapon and resistance",
		ABILITY_DBNPOIS			=> "poison breath weapon and resistance",
		ABILITY_GNOCUNNING		=> "gnome cunning",
		ABILITY_GNOCANTRIP		=> "natural illusionist",
		ABILITY_HELSKILLPROF		=> "skill versatility",
		ABILITY_HORMENACE		=> "menacing",
		ABILITY_HORRELEND		=> "relentless endurance",
		ABILITY_HORSAVATKS		=> "savage attacks",
		ABILITY_TIEFIRERES		=> "hellish resistance",
		ABILITY_TIECANTRIP		=> "infernal legacy",
		_				=> "unknown ability",
	};
	return description.to_string();
}

fn get_racial_stat_adjustments(race: &String) -> (i64, i64, i64, i64, i64, i64) {
	match &race[..] {
		"human"			=> (1, 1, 1, 1, 1, 1),
		"highelf"		=> (0, 0, 2, 1, 0, 0),
		"woodelf"		=> (0, 0, 2, 0, 1, 0),
		"darkelf"		=> (0, 0, 2, 0, 0, 1),
		"hilldwarf"		=> (0, 2, 0, 0, 1, 0),
		"mountaindwarf" 	=> (2, 2, 0, 0, 0, 0),
		"lightfoothalfling"	=> (0, 0, 2, 0, 0, 1),
		"stouthalfling"		=> (0, 1, 2, 0, 0, 0),
		"blackdragonborn"	=> (2, 0, 0, 0, 0, 1),
		"bluedragonborn"	=> (2, 0, 0, 0, 0, 1),
		"brassdragonborn"	=> (2, 0, 0, 0, 0, 1),
		"bronzedragonborn"	=> (2, 0, 0, 0, 0, 1),
		"copperdragonborn"	=> (2, 0, 0, 0, 0, 1),
		"golddragonborn"	=> (2, 0, 0, 0, 0, 1),
		"greendragonborn"	=> (2, 0, 0, 0, 0, 1),
		"reddragonborn"		=> (2, 0, 0, 0, 0, 1),
		"silverdragonborn"	=> (2, 0, 0, 0, 0, 1),
		"whitedragonborn"	=> (2, 0, 0, 0, 0, 1),
		"forestgnome"		=> (0, 0, 1, 2, 0, 0),
		"rockgnome"		=> (0, 1, 0, 2, 0, 0),
		"halforc"		=> (2, 1, 0, 0, 0, 0),
		"tiefling"		=> (0, 0, 0, 1, 0, 2),
		"halfelf"		=> halfelfpicker(),	
		_			=> (0, 0, 0, 0, 0, 0),
	}
}

fn halfelfpicker() -> (i64, i64, i64, i64, i64, i64) {
	let mut rng = rand::thread_rng();
	let rnd = rng.gen::<u64>();
	let num: i64 = ((rnd % 5) + 1) as i64;
	match num {
		1 => (1, 0, 0, 0, 0, 2),
		2 => (0, 1, 0, 0, 0, 2),
		3 => (0, 0, 1, 0, 0, 2),
		4 => (0, 0, 0, 1, 0, 2),
		5 => (0, 0, 0, 0, 1, 2),
		_ => (1, 0, 0, 0, 0, 2),
	}
}

fn get_racial_abilities(race: &String) -> u64 {
	match &race[..] {
		"human"			=> 0_u64,
		"highelf"		=> ABILITY_DARKVISION | ABILITY_ELFKEENSENS | ABILITY_ELFFEYANCEST | ABILITY_ELFCOMBATTRAIN | ABILITY_ELFCANTRIP,
		"woodelf"		=> ABILITY_DARKVISION | ABILITY_ELFKEENSENS | ABILITY_ELFFEYANCEST | ABILITY_ELFCOMBATTRAIN | ABILITY_ELFMOTW,
		"darkelf"		=> ABILITY_DARKVISION | ABILITY_ELFKEENSENS | ABILITY_ELFFEYANCEST | ABILITY_ELFCOMBATTRAIN | ABILITY_DELFSUNBAD | ABILITY_DELFMAGIC | ABILITY_DELFWEAPON | ABILITY_SUPDARKVISION,
		"hilldwarf"		=> ABILITY_DARKVISION | ABILITY_DWFRESILIENCE | ABILITY_DWFCOMBATTRAIN | ABILITY_DWFTOOLPROF | ABILITY_DWFTOUGHNESS,
		"mountaindwarf"		=> ABILITY_DARKVISION | ABILITY_DWFRESILIENCE | ABILITY_DWFCOMBATTRAIN | ABILITY_DWFTOOLPROF | ABILITY_DWFARMTRAIN,
		"lightfoothalfling"	=> ABILITY_HFLLUCK | ABILITY_HFLBRAVE | ABILITY_HFLSTEALTH,
		"stouthalfling"		=> ABILITY_HFLLUCK | ABILITY_HFLBRAVE | ABILITY_HFLRESILIENCE,
		"blackdragonborn"	=> ABILITY_DBNACID,
		"bluedragonborn"	=> ABILITY_DBNLIGHT,
		"brassdragonborn"	=> ABILITY_DBNFIRE,
		"bronzedragonborn"	=> ABILITY_DBNLIGHT,
		"copperdragonborn"	=> ABILITY_DBNACID,
		"golddragonborn"	=> ABILITY_DBNFIRE,
		"greendragonborn"	=> ABILITY_DBNPOIS,
		"reddragonborn"		=> ABILITY_DBNFIRE,
		"silverdragonborn"	=> ABILITY_DBNCOLD,
		"whitedragonborn"	=> ABILITY_DBNCOLD,
		_			=> 0_u64,
	}
}

fn read_abilities(flags: u64) -> String {
	let mut abilities = Vec::new();
	let mut position: u64 = 1;
	while position < ABILITY_NONE {
		if flags & position != 0_u64 {
			let text = get_ability_description(position);
			abilities.push(text);
		}
		position = position * 2;
	}
	let combined = abilities.join(", ").to_string();
	return combined;
}

fn is_race_set(conn: &Connection, nick: &String) -> bool {
	let set: i32 = conn.query_row("SELECT 1 FROM players WHERE nick = $1 AND race IS NOT NULL", &[&nick.as_str()], |row| {
		row.get(0)
	}).unwrap_or(0);
	if set != 0 {
		return true;
	}
	false
}

fn is_class_set(conn: &Connection, nick: &String) -> bool {
	let set: i32 = conn.query_row("SELECT 1 FROM players WHERE nick = $1 AND class IS NOT NULL", &[&nick.as_str()], |row| {
		row.get(0)
	}).unwrap_or(0);
	if set != 0 {
		return true;
	}

	false
}





