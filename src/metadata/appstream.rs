use crate::backend::traits::host_arch;
use crate::core::models::{App, Scope, is_valid_app_id};
use crate::error::{Error, Result};
use flate2::read::GzDecoder;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Parse one AppStream file. Returns the applications and how many invalid records were skipped.
pub fn load_file(path: &Path, remote: &str, scope: Scope, arch: &str) -> Result<(Vec<App>, usize)> {
    let f = File::open(path).map_err(|e| Error::Metadata(format!("{}: {e}", path.display())))?;
    if path.extension().is_some_and(|e| e == "gz") {
        parse(
            BufReader::with_capacity(1 << 16, GzDecoder::new(f)),
            remote,
            scope,
            arch,
        )
    } else {
        parse(BufReader::with_capacity(1 << 16, f), remote, scope, arch)
    }
}

#[derive(Default)]
struct Rec {
    kind: String,
    id: String,
    name: String,
    summary: String,
    desc: String,
    keywords: Vec<String>,
    homepage: Option<String>,
    developer: Option<String>,
    bundle: Option<String>,
}

fn has_lang(e: &BytesStart) -> bool {
    e.attributes()
        .flatten()
        .any(|a| a.key.as_ref() == "xml:lang")
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| {
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.into_owned())
        })
}

fn finish(r: Rec, remote: &str, scope: Scope, host: &str) -> Option<App> {
    // "desktop" is AppStream's legacy name for desktop-application (Fedora uses it for everything).
    if !["desktop-application", "console-application", "desktop"].contains(&r.kind.as_str()) {
        return None; // addons, runtimes, locales...: not installable applications
    }
    // `app/ID/ARCH/BRANCH` from the flatpak bundle is authoritative for identity.
    let (mut id, mut arch, mut branch) = (r.id.trim().to_string(), None, None);
    if let Some(b) = &r.bundle {
        let p: Vec<&str> = b.trim().split('/').collect();
        if p.len() == 4 && p[0] == "app" {
            id = p[1].to_string();
            arch = Some(p[2].to_string());
            branch = Some(p[3].to_string());
        }
    }
    if !is_valid_app_id(&id) {
        return Some(App::default()); // marker: invalid record, counted by the caller
    }
    if arch.as_deref().is_some_and(|a| a != host) {
        return None;
    }
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(App {
        id,
        name: squash(&r.name),
        summary: squash(&r.summary),
        description: squash(&r.desc),
        keywords: r.keywords,
        homepage: r.homepage,
        developer: r.developer,
        remote: remote.to_string(),
        branch,
        arch: arch.or_else(|| Some(host.to_string())),
        scope,
    })
}

pub fn parse(
    reader: impl BufRead,
    remote: &str,
    scope: Scope,
    arch: &str,
) -> Result<(Vec<App>, usize)> {
    let host = if arch.is_empty() { host_arch() } else { arch };
    let mut xml = Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut skip_at: Option<usize> = None;
    let mut rec: Option<Rec> = None;
    let (mut homepage_url, mut flatpak_bundle) = (false, false);
    let mut apps = Vec::new();
    let mut invalid = 0;

    loop {
        let ev = xml.read_event_into(&mut buf).map_err(|e| {
            Error::Metadata(format!(
                "malformed AppStream XML at byte {}: {e}",
                xml.buffer_position()
            ))
        })?;
        match ev {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_string();
                stack.push(name.clone());
                if skip_at.is_none() && stack.len() > 2 && has_lang(&e) {
                    skip_at = Some(stack.len());
                }
                if name == "component" && stack.len() == 2 {
                    rec = Some(Rec {
                        kind: attr(&e, "type").unwrap_or_else(|| "desktop-application".into()),
                        ..Rec::default()
                    });
                } else if rec.is_some() && stack.len() == 3 {
                    if name == "url" {
                        homepage_url = attr(&e, "type").as_deref() == Some("homepage");
                    } else if name == "bundle" {
                        flatpak_bundle = attr(&e, "type").as_deref() == Some("flatpak");
                    }
                }
                if let (Some(r), true) = (rec.as_mut(), skip_at.is_none())
                    && stack.len() > 3
                    && stack[2] == "description"
                {
                    r.desc.push(' '); // block boundary
                }
            }
            Event::End(_) => {
                if skip_at == Some(stack.len()) {
                    skip_at = None;
                }
                if stack.len() == 2
                    && let Some(r) = rec.take()
                {
                    match finish(r, remote, scope, host) {
                        Some(a) if a.id.is_empty() => invalid += 1,
                        Some(a) => apps.push(a),
                        None => {}
                    }
                }
                stack.pop();
            }
            Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => {
                if skip_at.is_some() || rec.is_none() || stack.len() < 3 {
                    buf.clear();
                    continue;
                }
                let text: String = match &ev {
                    Event::Text(t) => t.xml10_content().into_owned(),
                    Event::CData(c) => c.xml10_content().into_owned(),
                    Event::GeneralRef(g) => match g.resolve_char_ref() {
                        Ok(Some(c)) => c.to_string(),
                        _ => match &**g {
                            "amp" => "&".into(),
                            "lt" => "<".into(),
                            "gt" => ">".into(),
                            "quot" => "\"".into(),
                            "apos" => "'".into(),
                            _ => String::new(),
                        },
                    },
                    _ => unreachable!(),
                };
                let r = rec.as_mut().unwrap();
                let s: Vec<&str> = stack.iter().map(|v| v.as_str()).collect();
                match s.as_slice() {
                    [_, _, "id"] => r.id.push_str(&text),
                    [_, _, "name"] => r.name.push_str(&text),
                    [_, _, "summary"] => r.summary.push_str(&text),
                    [_, _, "url"] if homepage_url => {
                        r.homepage.get_or_insert_with(String::new).push_str(&text)
                    }
                    [_, _, "developer_name"] | [_, _, "developer", "name"] => {
                        r.developer.get_or_insert_with(String::new).push_str(&text)
                    }
                    [_, _, "bundle"] if flatpak_bundle => {
                        r.bundle.get_or_insert_with(String::new).push_str(&text)
                    }
                    [_, _, "keywords", "keyword"] => {
                        let t = text.trim();
                        if !t.is_empty() {
                            r.keywords.push(t.to_string());
                        }
                    }
                    [_, _, "description", ..] => r.desc.push_str(&text),
                    _ => {}
                }
            }
            Event::Eof => {
                if !stack.is_empty() {
                    return Err(Error::Metadata(
                        "truncated AppStream XML (unexpected end of file)".into(),
                    ));
                }
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    Ok((apps, invalid))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<components version="0.8" origin="flatpak">
  <component type="desktop-application">
    <id>org.mozilla.firefox</id>
    <name>Firefox</name>
    <name xml:lang="de">Feuerfuchs</name>
    <summary>Fast, Private &amp; Safe Web Browser</summary>
    <summary xml:lang="pl">Polski</summary>
    <developer_name>Mozilla</developer_name>
    <description><p>A <em>fast</em> browser.</p><ul><li>One</li><li>Two</li></ul></description>
    <description xml:lang="pl"><p>Po polsku</p></description>
    <url type="homepage">https://www.mozilla.org</url>
    <url type="bugtracker">https://bugs.example</url>
    <keywords><keyword>Browser</keyword><keyword xml:lang="ar">متصفح</keyword><keyword>Web</keyword></keywords>
    <bundle type="flatpak" runtime="x/y/z">app/org.mozilla.firefox/x86_64/stable</bundle>
  </component>
  <component type="desktop"><id>org.legacy.Desk</id><name>Legacy</name></component>
  <component type="runtime"><id>org.gnome.Platform</id><name>GNOME</name></component>
  <component type="addon"><id>org.a.Addon</id><name>Addon</name></component>
  <component type="desktop-application"><id>not valid id</id><name>Broken</name></component>
  <component type="console-application"><id>org.x.Cli</id><name>Cli</name></component>
  <component type="desktop-application">
    <id>org.arm.Only</id><name>Arm</name>
    <bundle type="flatpak">app/org.arm.Only/aarch64/stable</bundle>
  </component>
</components>"#;

    #[test]
    fn parses_fields_and_ignores_translations() {
        let (apps, invalid) = parse(XML.as_bytes(), "flathub", Scope::System, "x86_64").unwrap();
        assert_eq!(invalid, 1);
        assert_eq!(apps.len(), 3);
        let f = &apps[0];
        assert_eq!(f.id, "org.mozilla.firefox");
        assert_eq!(f.name, "Firefox");
        assert_eq!(f.summary, "Fast, Private & Safe Web Browser");
        assert_eq!(f.description, "A fast browser. One Two");
        assert_eq!(f.keywords, vec!["Browser", "Web"]);
        assert_eq!(f.homepage.as_deref(), Some("https://www.mozilla.org"));
        assert_eq!(f.developer.as_deref(), Some("Mozilla"));
        assert_eq!(f.branch.as_deref(), Some("stable"));
        assert_eq!(f.arch.as_deref(), Some("x86_64"));
        assert_eq!(f.remote, "flathub");
        assert_eq!(f.scope, Scope::System);
        assert_eq!(apps[1].id, "org.legacy.Desk");
        assert_eq!(apps[2].id, "org.x.Cli");
        assert_eq!(apps[2].branch, None); // no bundle: not fabricated
    }

    #[test]
    fn malformed_xml_is_an_error_not_an_empty_catalogue() {
        let r = parse(
            "<components><component><id>a.b</id>".as_bytes(),
            "r",
            Scope::User,
            "x86_64",
        );
        assert!(r.is_err(), "{r:?}");
        let r = parse(
            "<components><component></wrong></components>".as_bytes(),
            "r",
            Scope::User,
            "x86_64",
        );
        assert!(r.is_err());
    }

    #[test]
    fn gzip_files_are_read() {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("appstream.xml.gz");
        let mut e = GzEncoder::new(File::create(&p).unwrap(), Compression::fast());
        e.write_all(XML.as_bytes()).unwrap();
        e.finish().unwrap();
        let (apps, _) = load_file(&p, "fedora", Scope::System, "x86_64").unwrap();
        assert_eq!(apps.len(), 3);
    }
}
