//! Review tab: comments, spelling, accessibility; Slide Show requests.

mod slide_show;
use slide_show::{custom_show, setup, start};

mod proofing;
use proofing::{accessibility, spelling};

mod comments;
use comments::{add, delete, list, reply, resolve};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("comment.add", "New Comment", ["Review", "Comments"], Some("Cmd+Alt+M"), "{text, x?, y?, id?: shape}", has_slide, add),
        cmd!("comment.reply", "Reply", ["Comments"], None, "{index, text}", has_slide, reply),
        cmd!("comment.delete", "Delete Comment", ["Review", "Comments"], None, "{index? | all?: bool}", has_slide, delete),
        cmd!("comment.resolve", "Resolve Thread", ["Comments"], None, "{index, resolved?: bool}", has_slide, resolve),
        cmd!(query "comment.list", "Comments", [], None, "{slide?} → comments", has_slide, list),
        cmd!(query "review.accessibility", "Check Accessibility", ["Review", "Accessibility"], None, "{} → [{slide, shape, issue}]", has_doc, accessibility),
        cmd!(query "review.spelling", "Spelling", ["Review", "Proofing"], Some("F7"), "{} → [{slide, shape, word}] (words not in the built-in list)", has_doc, spelling),
        cmd!(noundo "show.fromStart", "Play from Start", ["Slide Show", "Start Slide Show"], Some("Cmd+Shift+Enter"), "{}", has_slide, |s, _| start(s, 0)),
        cmd!(noundo "show.fromCurrent", "Play from Current Slide", ["Slide Show", "Start Slide Show"], Some("Cmd+Enter"), "{}", has_slide, |s, _| {
            let i = s.doc()?.selection.slide;
            start(s, i)
        }),
        cmd!(
            "show.setup",
            "Set Up Slide Show…",
            ["Slide Show", "Set Up"],
            None,
            "{type?: speaker|browsed|kiosk, loop?: bool, noNarration?, noAnimation?, useTimings?, from?, to?}",
            has_doc,
            setup
        ),
        cmd!(
            "show.customShow",
            "Custom Slide Show",
            ["Slide Show", "Start Slide Show"],
            None,
            "{name, slides: [index]} | {name, delete: true}",
            has_doc,
            custom_show
        ),
    ]
}
