use crate::updater::UpdateResult;
use crate::updater::WebnovelProvider;

use epub::doc::EpubDoc;
use eyre::ContextCompat;
use eyre::{bail, eyre, Result};
use lazy_regex::regex;
use serde::Deserialize;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Deserialize)]
struct FanFicFareJson {
    output_filename: String,
}

pub struct FanFicFare;

impl FanFicFare {
    pub fn new(url: &str) -> Option<Self> {
        URLS.iter()
            .any(|compatible_url| url.contains(compatible_url))
            .then_some(Self)
    }
}

impl WebnovelProvider for FanFicFare {
    fn create(&self, dir: &Path, filename: Option<&str>, url: &str) -> Result<String> {
        let cmd = Command::new("fanficfare")
            .arg("--non-interactive")
            .arg("--json-meta")
            .arg(url)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Retrieve the metadata of the newly created book
        let stdout = cmd.stdout.ok_or_else(|| eyre!("Stdout is unavailable"))?;
        let book_metadata = BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .reduce(|accum, line| accum + &line)
            .ok_or_else(|| eyre!("Failed to read book metadata."))?;

        let generated_filename =
            serde_json::from_str::<FanFicFareJson>(&book_metadata).map(|e| e.output_filename)?;

        // Manage error cases
        let err_lines: String = cmd.stderr.map_or(String::new(), |stderr| {
            BufReader::new(stderr)
                .lines()
                .map_while(Result::ok)
                .collect()
        });

        if !err_lines.is_empty() {
            bail!("The execution of Fanficfare for '{url}'' ended with an error \n{err_lines}");
        }

        let mut file_path = dir.join(generated_filename);
        if let Some(filename) = filename {
            let new_file_path = dir.join(filename);
            fs::rename(file_path, &new_file_path)?;
            file_path = new_file_path;
        }

        let epub_doc = EpubDoc::new(&file_path)?;
        epub_doc.mdata("title").ok_or_else(|| eyre!("No title"))
    }

    fn update(&self, path: &Path) -> UpdateResult {
        do_update(path).into()
    }
}

fn do_update(path: &Path) -> Result<UpdateResult> {
    let updating = regex!(r"^Updating .*, URL: .*$");
    let up_to_date = regex!(r"^.* already contains \d+ chapters\.$");
    let do_update = regex!(r"^Do update - epub\((\d+)\) vs url\((\d+)\)$");
    let more_chapter_than_source =
        regex!(r"^.* contains (\d+) chapters, more than source: (\d+)\.$");
    let skipped = " - Skipping";

    let cmd = Command::new("fanficfare")
        .arg("--non-interactive")
        .arg("--update-epub")
        .arg("--update-cover")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = cmd.stdout.wrap_err(eyre!("No stdout"))?;
    let stderr = cmd.stderr.wrap_err(eyre!("No stderr"))?;
    let update_result = BufReader::new(stderr)
        .lines()
        .chain(BufReader::new(stdout).lines())
        .map_while(Result::ok)
        .filter(|line| updating.captures(line).is_none())
        .find_map(|line| {
            if up_to_date.captures(&line).is_some() {
                return Some(UpdateResult::UpToDate);
            }
            if let Some(c) = do_update.captures(&line) {
                let nb_chapter_epub = &c[1].parse::<u16>().ok()?;
                let nb_chapter_url = &c[2].parse::<u16>().ok()?;
                return Some(UpdateResult::Updated(nb_chapter_url - nb_chapter_epub));
            }
            if let Some(c) = more_chapter_than_source.captures(&line) {
                let nb_chapter_epub = &c[1].parse::<u16>().ok()?;
                let nb_chapter_url = &c[2].parse::<u16>().ok()?;
                return Some(UpdateResult::MoreChapterThanSource(
                    nb_chapter_epub - nb_chapter_url,
                ));
            }
            if line.ends_with(skipped) {
                return Some(UpdateResult::Skipped);
            }
            None
        })
        .wrap_err(eyre!("Could not parse FanFicFare output"))?;

    Ok(update_result)
}

const URLS: [&str; 166] = [
    "archiveofourown.org",
    "ashwinder.sycophanthex.com",
    "bloodshedverse.com",
    "chaos.sycophanthex.com",
    "chireads.com",
    "chosentwofanfic.com",
    "dark-solace.org",
    "efpfanfic.net",
    "erosnsappho.sycophanthex.com",
    "fanfics.me",
    "fanfictalk.com",
    "fanfictions.fr",
    "fastnovels.net",
    "ficbook.net",
    "fiction.live",
    "fictionhunt.com",
    "fictionhunt.com",
    "fictionmania.tv",
    "ficwad.com",
    "finestories.com",
    "forum.questionablequesting.com",
    "forums.spacebattles.com",
    "forums.sufficientvelocity.com",
    "gluttonyfiction.com",
    "imagine.e-fic.com",
    "inkbunny.net",
    "kakuyomu.jp",
    "ksarchive.com",
    "lcfanfic.com",
    "www.literotica.com",
    "www.literotica.com",
    "portuguese.literotica.com",
    "german.literotica.com",
    "lumos.sycophanthex.com",
    "mcstories.com",
    "mtt.just-once.net",
    "ncisfiction.com",
    "ninelivesarchive.com",
    "novelonlinefull.com",
    "occlumency.sycophanthex.com",
    "ponyfictionarchive.net",
    "explicit.ponyfictionarchive.net",
    "www.quotev.com",
    "readonlymind.com",
    "samandjack.net",
    "scifistories.com",
    "sheppardweir.com",
    "sinful-dreams.com",
    "spikeluver.com",
    "squidgeworld.org",
    "starslibrary.net",
    "storiesonline.net",
    "ncode.syosetu.com",
    "novel18.syosetu.com",
    "ncode.syosetu.com",
    "novel18.syosetu.com",
    "t.evancurrie.ca",
    "test1.com",
    "tgstorytime.com",
    "thehookupzone.net",
    "themasque.net",
    "touchfluffytail.org",
    "trekfanfiction.net",
    "valentchamber.com",
    "voracity2.e-fic.com",
    "www.adastrafanfic.com",
    "anime.adult-fanfiction.org",
    "anime2.adult-fanfiction.org",
    "bleach.adult-fanfiction.org",
    "books.adult-fanfiction.org",
    "buffy.adult-fanfiction.org",
    "cartoon.adult-fanfiction.org",
    "celeb.adult-fanfiction.org",
    "comics.adult-fanfiction.org",
    "ff.adult-fanfiction.org",
    "games.adult-fanfiction.org",
    "hp.adult-fanfiction.org",
    "inu.adult-fanfiction.org",
    "lotr.adult-fanfiction.org",
    "manga.adult-fanfiction.org",
    "movies.adult-fanfiction.org",
    "naruto.adult-fanfiction.org",
    "ne.adult-fanfiction.org",
    "original.adult-fanfiction.org",
    "tv.adult-fanfiction.org",
    "xmen.adult-fanfiction.org",
    "ygo.adult-fanfiction.org",
    "yuyu.adult-fanfiction.org",
    "www.alternatehistory.com",
    "www.aneroticstory.com",
    "www.asexstories.com",
    "www.asianfanfics.com",
    "www.bdsmlibrary.com",
    "www.deviantart.com",
    "www.dokuga.com",
    "www.dracoandginny.com",
    "aaran-st-vines.nsns.fanficauthors.net",
    "abraxan.fanficauthors.net",
    "bobmin.fanficauthors.net",
    "canoncansodoff.fanficauthors.net",
    "chemprof.fanficauthors.net",
    "copperbadge.fanficauthors.net",
    "crys.fanficauthors.net",
    "deluded-musings.fanficauthors.net",
    "draco664.fanficauthors.net",
    "fp.fanficauthors.net",
    "frenchsession.fanficauthors.net",
    "ishtar.fanficauthors.net",
    "jbern.fanficauthors.net",
    "jeconais.fanficauthors.net",
    "kinsfire.fanficauthors.net",
    "kokopelli.nsns.fanficauthors.net",
    "ladya.nsns.fanficauthors.net",
    "lorddwar.fanficauthors.net",
    "mrintel.nsns.fanficauthors.net",
    "musings-of-apathy.fanficauthors.net",
    "ruskbyte.fanficauthors.net",
    "seelvor.fanficauthors.net",
    "tenhawk.fanficauthors.net",
    "viridian.fanficauthors.net",
    "whydoyouneedtoknow.fanficauthors.net",
    "www.fanfiction.net",
    "www.fanfiction.net",
    "m.fanfiction.net",
    "www.fanfiktion.de",
    "www.fictionalley-archive.org",
    "www.fictionpress.com",
    "www.fictionpress.com",
    "m.fictionpress.com",
    "www.fimfiction.net",
    "www.fimfiction.com",
    "mobile.fimfiction.net",
    "www.fireflyfans.net",
    "www.giantessworld.net",
    "www.hentai-foundry.com",
    "www.libraryofmoria.com",
    "www.masseffect2.in",
    "www.mediaminer.org",
    "www.midnightwhispers.net",
    "www.mugglenetfanfiction.com",
    "www.naiceanilme.net",
    "www.narutofic.org",
    "www.novelall.com",
    "www.novelupdates.cc",
    "www.phoenixsong.net",
    "www.potionsandsnitches.org",
    "www.pretendercentre.com",
    "www.psychfic.com",
    "www.royalroad.com",
    "www.scribblehub.com",
    "www.siye.co.uk",
    "www.spiritfanfiction.com",
    "www.starskyhutcharchive.net",
    "www.storiesofarda.com",
    "www.sunnydaleafterdark.com",
    "www.swi.org.ru",
    "www.the-sietch.com",
    "www.thedelphicexpanse.com",
    "www.tthfanfic.org",
    "www.twilighted.net",
    "www.utopiastories.com",
    "www.walkingtheplank.org",
    "www.wattpad.com",
    "www.whofic.com",
    "www.wolverineandrogue.com",
    "www.wuxiaworld.xyz",
];
