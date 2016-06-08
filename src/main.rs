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
#[macro_use]
extern crate lazy_static;

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

#[derive(Debug)]
enum HitRoll {
	Crit,
	Fumble,
	Hit,
	Miss,
}

#[derive(Debug)]
enum Advantage {
	Advantage,
	Disadvantage,
	Normal,
}

#[derive(Debug)]
struct InventorySlot {
	slot_id: u64,
	item_id: i64,
}

#[derive(Debug)]
struct Player {
	uid: i64,
	nick: String,
	race: String,
	class: String,
	level: i64,
	strength: i32,
	constitution: i32,
	dexterity: i32,
	intelligence: i32,
	wisdom: i32,
	charisma: i32,
	racial_abilities: u64,
	class_abilities: u64,
	spell_effects: u64,
	inventory: Vec<InventorySlot>,
	max_hp: u64,
	current_hp: u64,
	fumbled: i8,
	init_mod: i8,
	tocrit: i8,
	spelleffects: Vec<Effect>,
	status: Vec<Status>,
	resistances: u64,
	dw_bonus: bool,
}

#[derive(Debug)]
struct Item {
	item_id: i64,
	name: String,
	description: String,
	value: i64,
	attributes: u64,
	slots: i16,
	damage_base: String,
	damage_base_type: u64,
	additional_damage_1: String,
	additional_damage_1_type: u64,
	additional_damage_2: String,
	additional_damage_2_type: u64,
	ac: i8,
	triggered_magic_effect: u64,
	triggered_magic_charges: u64,
	constant_magic_effect: u64,
	required_attribute: i8,
	required_attribute_min: i8,
}

// For spell effects. Use Status for mundane effects.
#[derive(Debug)]
struct Effect {
	effect_mask: u64,
	duration: u64, // rounds
	expires: i64, // time
}

#[derive(Debug)]
struct Status {
	status_mask: u64,
	duration: u64, // rounds
	expires: i64, // time
}

#[derive(Debug)]
struct DamageInfo {
	dice: String,
	adjustment: i8,
	damagetype: u64,
}

/*let mut stmt = conn.prepare(format!("SELECT * FROM messages WHERE recipient = '{}' ORDER BY ts", &nick).as_str()).unwrap();
	let mut allrows = stmt.query_map(&[], |row| {
		Row {
			sender: row.get(0),
			//recipient: row.get(1),
			message: row.get(2),
			ts: row.get(3)
		}
	}).unwrap();*/

lazy_static! {
	static ref ITEMS: Vec<Item> = {
		let mut items: Vec<Item> = Vec::new();
		let conn = Connection::open("/home/bob/etc/snbot/usersettings.db").unwrap();
		let mut stmt = conn.prepare(format!("SELECT * FROM items ORDER BY itemid").as_str()).unwrap();
		let mut allrows = stmt.query_map(&[], |row| {
			let attributes: i64 = row.get(4);
			let slots: i32 = row.get(5);
			let damage_base_type: i64 = row.get(7);
			let additional_damage_1_type: i64 = row.get(9);
			let additional_damage_2_type: i64 = row.get(11);
			let ac: i32 = row.get(12);
			let triggered_magic_effect: i64 = row.get(13);
			let triggered_magic_charges: i64 = row.get(14);
			let constant_magic_effect: i64 = row.get(15);
			let required_attribute: i32 = row.get(16);
			let required_attribute_min: i32 = row.get(17);
			let range: i32 = row.get(18);
			let special: i64 = row.get(19);
			Item {
				item_id: row.get(0),
				name: row.get(1),
				description: row.get(2),
				value: row.get(3),
				attributes: attributes as u64,
				slots: slots as i16,
				damage_base: row.get(6),
				damage_base_type: damage_base_type as u64,
				additional_damage_1: row.get(8),
				additional_damage_1_type: additional_damage_1_type as u64,
				additional_damage_2: row.get(10),
				additional_damage_2_type: additional_damage_2_type as u64,
				ac: ac as i8,
				triggered_magic_effect: triggered_magic_effect as u64,
				triggered_magic_charges: triggered_magic_charges as u64,
				constant_magic_effect: constant_magic_effect as u64,
				required_attribute: required_attribute as i8,
				required_attribute_min: required_attribute_min as i8,
				range: range as u16,
				special: special as u64,
			}
		}).unwrap();
		conn.close();
		for row in allrows {
			let item = row.unwrap();
			items.push(item);
		}
		items
	};
}

const DEBUG: bool = false;
const ITEM_MONEY: i64 = 1_i64;
const ITEM_NONE: i64 = 0_i64;
static ITEM_ITEM_NONE: Item = Item {
	item_id: 0_i64,
	name: "".to_string(),
	description: "".to_string(),
	value: 0_i64,
	attributes: 0_u64,
	slots: 0_i16,
	damage_base: "0d1".to_string(),
	damage_base_type: 0_u64,
	additional_damage_1: "".to_string(),
	additional_damage_1_type: 0_u64,
	additional_damage_2: "".to_string(),
	additional_damage_2_type: 0_u64,
	ac: 0_i8,
	triggered_magic_effect: 0_u64,
	triggered_magic_charges: 0_u64,
	constant_magic_effect: 0_u64,
	required_attribute: 0_i8,
	required_attribute_min: 0_i8,
	range: 0_u16,
	special: 0_u64,
};
const RACES: &'static [&'static str] = &["human", "highelf", "woodelf", "darkelf", "hilldwarf", "mountaindwarf", "lightfoothalfling", "stouthalfling", "blackdragonborn", "bluedragonborn", "brassdragonborn", "bronzedragonborn", "copperdragonborn", "golddragonborn", "greendragonborn", "reddragonborn", "silverdragonborn", "whitedragonborn", "forestgnome", "rockgnome", "halfelf", "halforc", "tiefling"];
const CLASSES: &'static [&'static str] = &["barbarian", "bard", "cleric", "druid", "fighter", "monk", "paladin", "ranger", "rogue", "sorcerer", "warlock", "wizard"];

// STAT ADJUSTMENTS
const STAT_BONUSES: [i8; 31] = [-5, -5, -4, -4, -3, -3, -2, -2, -1, -1, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10];

// MISC
const PRIMARY_HAND: u64 = 1;
const SECONDARY_HAND: u64 = 2;

// RACIAL ABILITIES
const ABILITY_DARKVISION: u64 = 1;
const ABILITY_SUPDARKVISION: u64 = 2;
const ABILITY_DWFRESILIENCE: u64 = 4;
const ABILITY_DWFCOMBATTRAIN: u64 = 8;
const ABILITY_DWFTOOLPROF: u64 = 16;
const ABILITY_DWFTOUGHNESS: u64 = 32;
const ABILITY_DWFARMTRAIN: u64 = 64;
const ABILITY_ELFKEENSENS: u64 = 128;
const ABILITY_ELFFEYANCEST: u64 = 256;
const ABILITY_ELFCOMBATTRAIN: u64 = 512;
const ABILITY_ELFCANTRIP: u64 = 1024;
const ABILITY_ELFMOTW: u64 = 2048;
const ABILITY_DELFSUNBAD: u64 = 4096;
const ABILITY_DELFMAGIC: u64 = 8192;
const ABILITY_DELFWEAPON: u64 = 16384;
const ABILITY_HFLLUCK: u64 = 32768;
const ABILITY_HFLBRAVE: u64 = 65536;
const ABILITY_HFLSTEALTH: u64 = 131072;
const ABILITY_HFLRESILIENCE: u64 = 262144;
const ABILITY_DBNFIRE: u64 = 524288;
const ABILITY_DBNCOLD: u64 = 1048576;
const ABILITY_DBNACID: u64 = 2097152;
const ABILITY_DBNLIGHT: u64 = 4194304;
const ABILITY_DBNPOIS: u64 = 8388608;
const ABILITY_GNOCUNNING: u64 = 16777216;
const ABILITY_GNOCANTRIP: u64 = 33554432;
const ABILITY_HELSKILLPROF: u64 = 67108864;
const ABILITY_HORMENACE: u64 = 134217728;
const ABILITY_HORRELEND: u64 = 268435456;
const ABILITY_HORSAVATKS: u64 = 536870912;
const ABILITY_TIEFIRERES: u64 = 1073741824;
const ABILITY_TIECANTRIP: u64 = 2147483648;
const ABILITY_NONE: u64 = 4294967296;

// CLASS ABILITIES

// MUNDANE STATUS EFFECTS
const STATUS_PRONE: u64 = 1;
const STATUS_ASLEEP: u64 = 2;
const STATUS_GRAPPLED: u64 = 4;
const STATUS_HIDDEN: u64 = 8;
const STATUS_BURNING: u64 = 16;
const STATUS_CALTROP: u64 = 32;
const STATUS_BLIND: u64 = 64;
const STATUS_DEAF: u64 = 128;
const STATUS_POISON: u64 = 256;
const STATUS_STUN: u64 = 512;
const STATUS_RESTRAIN: u64 = 1024;
const STATUS_MARBLES: u64 = 2048;

// MAGIC EFFECTS



// ITEM ATTRIBUTES
const ITEM_IS_WEAPON: u64 = 1;
const ITEM_IS_ARMOR: u64 = 2;
const ITEM_IS_MAGIC: u64 = 4;
const ITEM_PLUS_ONE: u64 = 8;
const ITEM_PLUS_TWO: u64 = 16;
const ITEM_PLUS_THREE: u64 = 32;
const ITEM_PLUS_FOUR: u64 = 64;
const ITEM_PLUS_FIVE: u64 = 128;
const ITEM_ADDITIONAL_DMG_1: u64 = 256;
const ITEM_ADDITIONAL_DMG_2: u64 = 512;
const ITEM_IS_SHIELD: u64 = 1024;
const ITEM_MAX_DEX_NONE: u64 = 2048;
const ITEM_MAX_DEX_2: u64 = 4096;
const ITEM_STEALTH_DISADVANTAGE: u64 = 8192;
const ITEM_PERM_EFFECT: u64 = 16384;
const ITEM_CHARGED: u64 = 32768;
const ITEM_FINESSE: u64 = 65536;
const ITEM_2HANDED: u64 = 131072;
const ITEM_THROWN: u64 = 262144;
const ITEM_RANGED: u64 = 524288;
const ITEM_AMMO: u64 = 1048576;
const ITEM_LIGHT: u64 = 2097152;
const ITEM_MEDIUM: u64 = 4194304;
const ITEM_HEAVY: u64 = 8388608;
const ITEM_VERSATILE: u64 = 16777216;
const ITEM_REACH: u64 = 33554432;
const ITEM_SPECIAL: u64 = 67108864;
const ITEM_SIMPLE: u64 = 134217728;
const ITEM_MARTIAL: u64 = 268435456;
const ITEM_ELF: u64 = 536870912;
const ITEM_THIEF: u64 = 1073741824;

// SPECIAL WEAPON CHARACTERISTICS
const SPECIAL_LANCE: u64 = 1;
const SPECIAL_NET: u64 = 2;

// DAMAGE TYPES
const DAMAGE_TYPE_SLASHING: u64 = 1;
const DAMAGE_TYPE_PIERCING: u64 = 2;
const DAMAGE_TYPE_BLUDGEONING: u64 = 4;
const DAMAGE_TYPE_FIRE: u64 = 8;
const DAMAGE_TYPE_ACID: u64 = 16;
const DAMAGE_TYPE_COLD: u64 = 32;
const DAMAGE_TYPE_LIGHTNING: u64 = 64;
const DAMAGE_TYPE_POISON: u64 = 128;
const DAMAGE_TYPE_FORCE: u64 = 256;
const DAMAGE_TYPE_NECROTIC: u64 = 512;
const DAMAGE_TYPE_RADIANT: u64 = 1024;
const DAMAGE_TYPE_PSYCHIC: u64 = 2048;
const DAMAGE_TYPE_THUNDER: u64 = 4096;
const DAMAGE_TYPE_MAGIC: u64 = 8192;

// INVENTORY SLOTS
const SLOT_PRIMARY_HAND: u64 = 1;
const SLOT_SECONDARY_HAND: u64 = 2;
const SLOT_ARMOR: u64 = 4;
const SLOT_HEAD: u64 = 8;
const SLOT_ARMS: u64 = 16;
const SLOT_HANDS: u64 = 32;
const SLOT_RIGHT_RING: u64 = 64;
const SLOT_LEFT_RING: u64 = 128;
const SLOT_FEET: u64 = 256;
const SLOT_BACKPACK: u64 = 512;
const SLOT_TWO_HANDED: u64 = 1024;
const SLOT_BELT_POUCH: u64 = 2048;
const SLOT_FACE: u64 = 4096;
const SLOT_CAPE: u64 = 8192;

// STATS
const STAT_STR: u8 = 1;
const STAT_CON: u8 = 2;
const STAT_DEX: u8 = 4;
const STAT_INT: u8 = 8;
const STAT_WIS: u8 = 16;
const STAT_CHA: u8 = 32;

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
	
	subtx.send(submission).unwrap();
}

fn command_quit(server: &IrcServer, chan: String) {
	server.send_privmsg(&chan, "Your wish is my command...");
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
			location = checklocation;
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
		"bot_config" => "CREATE TABLE bot_config(nick TEXT PRIMARY KEY, server TEXT, channel TEXT, perl_file TEXT, prefix TEXT, admin_hostmask TEXT, snpass TEXT, snuser TEXT, cookiefile TEXT, wu_api_key TEXT, google_key text)".to_string(),
		"locations" => "CREATE TABLE locations(nick TEXT PRIMARY KEY, location TEXT)".to_string(),
		"bots" => "CREATE TABLE bots(hostmask TEXT PRIMARY KEY NOT NULL)".to_string(),
		"abusers" => "CREATE TABLE abusers(hostmask TEXT PRIMARY KEY NOT NULL)".to_string(),
		"admins" => "CREATE TABLE admins(hostmask PRIMARY KEY NOT NULL)".to_string(),
		"test" => "CREATE TABLE test(hostmask PRIMARY KEY NOT NULL)".to_string(),
		"messages" => "CREATE TABLE messages(sender TEXT, recipient TEXT, message TEXT, ts UNSIGNED INT(8))".to_string(),
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

	let url = format!("http://api.wunderground.com/api/{}/forecast/q/{}.json", wu_key.to_string(), fix_location(&location).to_string());

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
		let citystate = format!("{}/{}", state.trim_left_matches(",").trim(), city.trim()).to_string();
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
	for regex in titleres.iter() {
		let captures = regex.captures(page.as_str());
		if captures.is_none() {
			return "".to_string();
		}
		let cap = captures.unwrap().at(1).unwrap_or("");
		if cap != "" {
			return cap.to_string().trim().to_string();
		}
	}
	return "".to_string();
}

fn sub_get_description(descres: &Vec<Regex>, page: &String) -> String {
	for regex in descres.iter() {
		let captures = regex.captures(page.as_str());
		if captures.is_none() {
			return "".to_string();
		}
		let cap = captures.unwrap().at(1).unwrap_or("");
		if cap != "" {
			return cap.to_string().trim().to_string();
		}
	}
	return "".to_string();
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

fn roll_initiative(adjustments: &i8) -> i8 {
	let mut rng = rand::thread_rng();
	let random = rng.gen::<u64>();
	let roll: i8 = ((random % 10) + 1) as i8 + *adjustments;
	return roll;
}

fn do_combat_round(mut attacker: &mut Player, mut defender: &mut Player) {
	let mut a_init = roll_initiative(&attacker.init_mod);
	let mut d_init = roll_initiative(&defender.init_mod);
	while a_init == d_init {
		a_init = roll_initiative(&attacker.init_mod);
		d_init = roll_initiative(&defender.init_mod);
	}
	
	if a_init > d_init {
		do_melee_attack(&mut attacker, &mut defender);
		if is_dead(&defender) {
			// Victory
			return;
		}
		do_melee_attack(&mut defender, &mut attacker);
		if is_dead(&attacker) {
			// Victory
			return;
		}
		return;
	}
	else {
		do_melee_attack(&mut defender, &mut attacker);
		if is_dead(&attacker) {
			// Victory
			return;
		}
		do_melee_attack(&mut attacker, &mut defender);
		if is_dead(&defender) {
			// Victory
			return;
		}
		return;
	}
}

// attacker/defender here do not mean the same as in do_combat_round
// Here attacker means whoever's turn it is to attack.
fn do_melee_attack(mut attacker: &mut Player, mut defender: &mut Player) {
	let mut phattacks: u64 = get_ph_attacks(&attacker) + 1;
	let advantage: Advantage;
	if player_has_advantage(&attacker) {
		advantage = Advantage::Advantage;
	}
	else if player_has_disadvantage(&attacker) {
		advantage = Advantage::Disadvantage;
	}
	else {
		advantage = Advantage::Normal;
	}
	let priweaponcheck = get_damage_formulas(&attacker, PRIMARY_HAND);
	let mut damageformulas: Vec<DamageInfo>;
	if priweaponcheck.is_some() {
		damageformulas = priweaponcheck.unwrap();
	}
	else { phattacks = 1; }
	for _ in 1..phattacks {
		let swing: HitRoll = roll_tohit(get_ac(&defender), get_att_adj(&attacker, PRIMARY_HAND), &attacker.tocrit, &advantage);
		match swing {
			HitRoll::Crit => {
				for formula in damageformulas.iter() {
					let damage: u32 = roll_damage(&formula.dice, &formula.adjustment) * 2;
					apply_damage(&mut defender, damage, &formula.damagetype);
				}
			},
			HitRoll::Fumble => { attacker.fumbled = 1; return; },
			HitRoll::Miss => {continue;},
			HitRoll::Hit => {
				for formula in damageformulas.iter() {
					let damage: u32 = roll_damage(&formula.dice, &formula.adjustment);
					apply_damage(&mut defender, damage, &formula.damagetype);
				}
			},
		};
	}

	let shattacks: u64 = get_sh_attacks(&attacker) + 1;
	if shattacks == 0_u64 { return; }
	let secondweaponcheck = get_damage_formulas(&attacker, SECONDARY_HAND);
	if secondweaponcheck.is_some() {
		damageformulas = secondweaponcheck.unwrap();
	}
	else { return; }
	for _ in 1..shattacks {
		let swing: HitRoll = roll_tohit(get_ac(&defender), get_att_adj(&attacker, SECONDARY_HAND), &attacker.tocrit, &advantage);
		match swing {
			HitRoll::Crit => {
				for formula in damageformulas.iter() {
					let damage: u32 = roll_damage(&formula.dice, &formula.adjustment) * 2;
					apply_damage(&mut defender, damage, &formula.damagetype);
				}
			},
			HitRoll::Fumble => { attacker.fumbled = 1; return; },
			HitRoll::Miss => {continue;},
			HitRoll::Hit => {
				for formula in damageformulas.iter() {
					let damage: u32 = roll_damage(&formula.dice, &formula.adjustment) * 2;
					apply_damage(&mut defender, damage, &formula.damagetype);
				}
			},
		};
	}
}

fn roll_tohit(ac: i8, adjustments: i8, tocrit: &i8, advantage: &Advantage) -> HitRoll {
	let result: HitRoll;
	let mut rng = rand::thread_rng();
	let random = rng.gen::<u64>();
	let random2 = rng.gen::<u64>();
	let mut rawroll: i8 = ((random % 20) + 1) as i8;
	let mut secondroll: i8 = ((random % 20) + 1) as i8;
	match advantage {
		&Advantage::Advantage => {
			if secondroll > rawroll {
				rawroll = secondroll;
			}
		},
		&Advantage::Disadvantage => {
			if secondroll < rawroll {
				rawroll = secondroll;
			}
		},
		&Advantage::Normal => {},
	};
	let roll: i8 = rawroll + adjustments;
	if rawroll >= *tocrit {
		result = HitRoll::Crit;
	}
	else if rawroll == 1 {
		result = HitRoll::Fumble;
	}
	else if roll >= ac {
		result = HitRoll::Hit;
	}
	else {
		result = HitRoll::Miss;
	}
	return result;
}

fn roll_damage(dice: &String, adjustments: &i8) -> u32 {
	let mut result: u32 = 0;
	let mut total: i32 = 0;
	let mut rng = rand::thread_rng();
	let mut dpos = dice.find("d").unwrap();
	let mut num: u64 = dice[..dpos].parse().unwrap();
	num += 1;
	dpos += 1;
	let sides: u64 = dice[dpos..].parse().unwrap();
	for _ in 1..num {
		let random = rng.gen::<u64>();
		let roll: u64 = (random % sides) + 1;
		total += roll as i32;
	}
	total += *adjustments as i32;
	if total <= 0 { return 0; }
	return total as u32;
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

fn player_has_advantage(player: &Player) -> bool {
	false
}

fn player_has_disadvantage(player: &Player) -> bool {
	false
}

fn is_dead(player: &Player) -> bool {
	if player.current_hp <= 0 {
		return true;
	}
	false
}

fn get_ph_attacks(player: &Player) -> u64 {
	return 1;
}

fn get_sh_attacks(player: &Player) -> u64 {
	return 0;
}

fn get_damage_formulas(player: &Player, hand: u64) -> Option<Vec<DamageInfo>> {
	let mut damages: Vec<DamageInfo> = Vec::new();
	let mut weapon: &Item = &ITEM_ITEM_NONE;
	let mut weapon_id = ITEM_NONE;
	for slot in player.inventory.iter() {
		if hand == PRIMARY_HAND {
			if slot.slot_id == SLOT_PRIMARY_HAND {
				weapon_id = slot.item_id;
				break;
			}
		}
		else if hand == SECONDARY_HAND {
			if slot.slot_id == SLOT_SECONDARY_HAND {
				weapon_id = slot.item_id;
				break;
			}
		}
	}
	for item in ITEMS.iter() {
		if item.item_id == weapon_id {
			weapon = &item;
		}
	}
	if weapon.attributes & ITEM_IS_WEAPON == 0 {
		return None;
	}
	// plusses first
	let magic: i8 = 0_i8;
	if weapon.attributes & ITEM_PLUS_ONE != 0 {
		magic += 1;
	}
	if weapon.attributes & ITEM_PLUS_TWO != 0 {
		magic += 2;
	}
	if weapon.attributes & ITEM_PLUS_THREE != 0 {
		magic += 3;
	}
	if weapon.attributes & ITEM_PLUS_FOUR != 0 {
		magic += 4;
	}
	if weapon.attributes & ITEM_PLUS_FIVE != 0 {
		magic += 5;
	}
	if magic != 0 {
		let magical = DamageInfo {
			dice: "0d1".to_string(),
			adjustment: magic,
			damagetype: DAMAGE_TYPE_MAGIC,
		};
		damages.push(magical);
	}

	// stat damage bonus/penalty

	// primary damage
	let (dice, plusses) = split_dmg_formula(&weapon.damage_base);
	let primary = DamageInfo {
			dice: dice.clone(),
			adjustment: plusses,
			damagetype: weapon.damage_base_type,
		};
	damages.push(primary);
	//1st elemental damage
	if weapon.attributes & ITEM_ADDITIONAL_DMG_1 != 0 {
		let (dice, plusses) = split_dmg_formula(&weapon.additional_damage_1);
		let elem1 = DamageInfo {
				dice: dice.clone(),
				adjustment: plusses,
				damagetype: weapon.additional_damage_1_type,
		};
		damages.push(elem1);
	}
	//2nd elemental damage
	if weapon.attributes & ITEM_ADDITIONAL_DMG_2 != 0 {
		let (dice, plusses) = split_dmg_formula(&weapon.additional_damage_2);
		let elem2 = DamageInfo {
				dice: dice.clone(),
				adjustment: plusses,
				damagetype: weapon.additional_damage_2_type,
		};
		damages.push(elem2);
	}
	return Some(damages);
}

// armor needs to be set to ITEM_ITEM_NONE
fn get_ac(player: &Player) -> i8 {
	let gear_ac = get_gear_ac(&player);
	let mut armor: &Item = &ITEM_ITEM_NONE;
	let mut armor_id = ITEM_NONE;
	for slot in player.inventory.iter() {
		if slot.slot_id == SLOT_ARMOR {
			armor_id = slot.item_id;
			break;
		}
	}
	for item in ITEMS.iter() {
		if item.item_id == armor_id {
			armor = &item;
			break;
		}
	}
	let dex_bonus = get_effective_dex_bonus(&armor, STAT_BONUSES[player.dexterity as usize]);
	let total_ac = 10 + gear_ac + dex_bonus + get_spells_ac(&player);
	return total_ac;
}

fn get_effective_dex_bonus(armor: &Item, actual_bonus: i8) -> i8 {
	let none: u64 = ITEM_MAX_DEX_NONE;
	let two: u64 = ITEM_MAX_DEX_2;
	if actual_bonus <= 0 {
		return actual_bonus;
	}
	if armor.attributes & none != 0_u64 {
		return actual_bonus;
	}
	if armor.attributes & two != 0_u64  && actual_bonus > 2 {
		return 2;
	}
	if armor.attributes & two != 0_u64  && actual_bonus <= 2 {
		return actual_bonus;
	}
	if armor.attributes & none == 0_u64 && armor.attributes & two == 0_u64 && actual_bonus >= 0 {
		return 0;
	}
	return 0;
}

fn get_gear_ac(player: &Player) -> i8 {
	let mut ac: i8 = 0;
	for slot in player.inventory.iter() {
		if slot.slot_id == SLOT_BACKPACK || slot.slot_id == SLOT_BELT_POUCH {
			continue;
		}
		let item: &Item;
		for this in ITEMS.iter() {
			if this.item_id == slot.item_id {
				item = &this;
				break;
			}
		}
		ac += item.ac;
		if item.attributes & ITEM_PLUS_ONE != 0  && item.attributes & ITEM_IS_WEAPON == 0 {
			ac += 1;
		}
		if item.attributes & ITEM_PLUS_TWO != 0  && item.attributes & ITEM_IS_WEAPON == 0 {
			ac += 2;
		}
		if item.attributes & ITEM_PLUS_THREE != 0  && item.attributes & ITEM_IS_WEAPON == 0 {
			ac += 3;
		}
		if item.attributes & ITEM_PLUS_FOUR != 0  && item.attributes & ITEM_IS_WEAPON == 0 {
			ac += 4;
		}
		if item.attributes & ITEM_PLUS_FIVE != 0  && item.attributes & ITEM_IS_WEAPON == 0 {
			ac += 5;
		}
	}
	return ac;
}

fn get_spells_ac(player: &Player) -> i8 {
	return 0_i8;
}

fn apply_damage(mut player: &mut Player, damage: u32, damagetype: &u64) {
	if player.resistances & damagetype != 0 {
		if player.current_hp >= ((damage / 2) as u64) {
			player.current_hp = player.current_hp - ((damage / 2) as u64);
		}
		else {
			player.current_hp = 0u64;
		}
	}
	else {
		if player.current_hp >= (damage as u64) {
			player.current_hp = player.current_hp - (damage as u64);
		}
		else {
			player.current_hp = 0u64;
		}			
	}
}

fn split_dmg_formula(formula: &String) -> (String, i8) {
	let mut plus = formula.find("+").unwrap_or(0_usize);
	if plus == 0_usize {
		let fclone = formula.clone();
		return (fclone, 0_i8);
	}
	let dice = formula[..plus].trim().to_string();
	plus += 1;
	let plusses: i8 = formula[plus..].trim().to_string().parse::<i8>().unwrap();
	return (dice, plusses);
}

// need to add proficiency and magic checking in here
fn get_att_adj(player: &Player, hand: u64) -> i8 {
	let stradj = STAT_BONUSES[player.strength];
	let dexadj = STAT_BONUSES[player.dexterity];
	let mut useadj: i8 = 0_i8;
	
	let mut weapon: &Item = &ITEM_ITEM_NONE;
	let mut weapon_id = ITEM_NONE;
	for slot in player.inventory.iter() {
		if hand == PRIMARY_HAND {
			if slot.slot_id == SLOT_PRIMARY_HAND {
				weapon_id = slot.item_id;
				break;
			}
		}
		else if hand == SECONDARY_HAND {
			if slot.slot_id == SLOT_SECONDARY_HAND {
				weapon_id = slot.item_id;
				break;
			}
		}
	}
	for item in ITEMS.iter() {
		if item.item_id == weapon_id {
			weapon = &item;
		}
	}
	
	if weapon.attributes & ITEM_IS_WEAPON == 0 {
		return 0_i8;
	}
	if player.dw_bonus == true {
		if weapon.attributes & ITEM_FINESSE == 0 {
			useadj = stradj;
		}
		else {
			if dexadj > stradj {
				useadj = dexadj;
			}
			else {
				useadj = stradj;
			}
		}
	}

	// now add magic bonuses
	if weapon.attributes & ITEM_PLUS_ONE != 0 {
		useadj += 1;
	}
	if weapon.attributes & ITEM_PLUS_TWO != 0 {
		useadj += 2;
	}
	if weapon.attributes & ITEM_PLUS_THREE != 0 {
		useadj += 3;
	}
	if weapon.attributes & ITEM_PLUS_FOUR != 0 {
		useadj += 4;
	}
	if weapon.attributes & ITEM_PLUS_FIVE != 0 {
		useadj += 5;
	}

	// now adjust for proficiency
	if player.proficienc
	return useadj;
}







