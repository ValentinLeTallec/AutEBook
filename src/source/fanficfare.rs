use super::Source;
use crate::updater::FanFicFare;
use crate::updater::WebNovel;

#[derive(Debug, PartialEq, Eq)]
pub struct FanFicFareCompatible {}

impl Source for FanFicFareCompatible {
    fn get_updater(&self) -> Option<Box<dyn WebNovel>> {
        Some(Box::new(FanFicFare::new()))
    }

    fn new(fiction_url: &str) -> Option<Self> {
        if URLS
            .iter()
            .any(|compatible_url| fiction_url.contains(compatible_url))
        {
            Some(Self {})
        } else {
            None
        }
    }
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
