// SPDX-License-Identifier: AGPL-3.0-or-later
//! BCF 3.0 (BIM Collaboration Format) export and import of issues, so they travel to and from other
//! BIM tools. Viewpoints carry the selected elements and a perspective camera.
//!
//! Camera coordinates: NicGit stores the web viewer's Y-up coordinates; BCF uses the model's Z-up axes.
//! Converted as IFC (x, y, z) = viewer (x, -z, y). The viewer re-centres large models, so a camera
//! exported from a georeferenced model is approximate (the selection is exact).
use quick_xml::{events::Event, Reader};
use serde_json::{json, Value};
use std::io::{Read, Write};

pub struct Topic {
    pub guid: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub author: String,
    pub assignee: Option<String>,
    pub created: String,
    pub comments: Vec<(String, String, String)>, // (author, date, text)
    pub viewpoint: Option<Value>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn v3(v: &Value, key: &str) -> Option<[f64; 3]> {
    let a = v.get(key)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

fn to_ifc(p: [f64; 3]) -> [f64; 3] {
    [p[0], -p[2], p[1]]
}

fn from_ifc(p: [f64; 3]) -> [f64; 3] {
    [p[0], p[2], -p[1]]
}

fn xyz(tag: &str, p: [f64; 3]) -> String {
    format!("<{tag}><X>{}</X><Y>{}</Y><Z>{}</Z></{tag}>", p[0], p[1], p[2])
}

fn viewpoint_xml(guid: &str, vp: &Value) -> String {
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<VisualizationInfo Guid=\"{guid}\">");
    let selected: Vec<&str> = vp["selected"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !selected.is_empty() {
        s.push_str("<Components><Selection>");
        for g in selected {
            s.push_str(&format!("<Component IfcGuid=\"{}\"/>", esc(g)));
        }
        s.push_str("</Selection></Components>");
    }
    let cam = &vp["camera"];
    if let (Some(pos), Some(dir), Some(up)) = (v3(cam, "position"), v3(cam, "direction"), v3(cam, "up")) {
        let fov = cam["fov"].as_f64().unwrap_or(45.0);
        s.push_str(&format!(
            "<PerspectiveCamera>{}{}{}<FieldOfView>{fov}</FieldOfView><AspectRatio>1.6</AspectRatio></PerspectiveCamera>",
            xyz("CameraViewPoint", to_ifc(pos)),
            xyz("CameraDirection", to_ifc(dir)),
            xyz("CameraUpVector", to_ifc(up))
        ));
    }
    s.push_str("</VisualizationInfo>\n");
    s
}

pub fn export(project_name: &str, topics: &[Topic]) -> anyhow::Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        z.start_file("bcf.version", opt)?;
        z.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Version VersionId=\"3.0\"/>\n")?;
        z.start_file("project.bcfp", opt)?;
        z.write_all(
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ProjectInfo><Project ProjectId=\"{}\"><Name>{}</Name></Project></ProjectInfo>\n",
                uuid::Uuid::new_v4(),
                esc(project_name)
            )
            .as_bytes(),
        )?;
        for t in topics {
            let mut m = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Markup><Topic Guid=\"{}\" TopicType=\"Issue\" TopicStatus=\"{}\"><Title>{}</Title><Priority>{}</Priority><CreationDate>{}</CreationDate><CreationAuthor>{}</CreationAuthor>",
                esc(&t.guid),
                esc(&t.status),
                esc(&t.title),
                esc(&t.priority),
                esc(&t.created),
                esc(&t.author)
            );
            if let Some(a) = &t.assignee {
                m.push_str(&format!("<AssignedTo>{}</AssignedTo>", esc(a)));
            }
            m.push_str(&format!("<Description>{}</Description>", esc(&t.description)));
            if !t.comments.is_empty() {
                m.push_str("<Comments>");
                for (author, date, text) in &t.comments {
                    m.push_str(&format!(
                        "<Comment Guid=\"{}\"><Date>{}</Date><Author>{}</Author><Comment>{}</Comment></Comment>",
                        uuid::Uuid::new_v4(),
                        esc(date),
                        esc(author),
                        esc(text)
                    ));
                }
                m.push_str("</Comments>");
            }
            let vp_guid = uuid::Uuid::new_v4().to_string();
            if t.viewpoint.is_some() {
                m.push_str(&format!("<Viewpoints><ViewPoint Guid=\"{vp_guid}\"><Viewpoint>viewpoint.bcfv</Viewpoint></ViewPoint></Viewpoints>"));
            }
            m.push_str("</Topic></Markup>\n");
            z.start_file(format!("{}/markup.bcf", t.guid), opt)?;
            z.write_all(m.as_bytes())?;
            if let Some(vp) = &t.viewpoint {
                z.start_file(format!("{}/viewpoint.bcfv", t.guid), opt)?;
                z.write_all(viewpoint_xml(&vp_guid, vp).as_bytes())?;
            }
        }
        z.finish()?;
    }
    Ok(buf.into_inner())
}

fn text_of(reader: &mut Reader<&[u8]>) -> String {
    let mut out = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(t)) => out.push_str(&t.unescape().map(|c| c.into_owned()).unwrap_or_default()),
            Ok(Event::CData(t)) => out.push_str(&String::from_utf8_lossy(&t)),
            _ => return out,
        }
    }
}

fn attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == name).and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

fn parse_markup(xml: &str) -> Option<Topic> {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(true);
    let mut t = Topic {
        guid: String::new(),
        title: String::new(),
        description: String::new(),
        status: "Open".into(),
        priority: "normal".into(),
        author: String::new(),
        assignee: None,
        created: String::new(),
        comments: vec![],
        viewpoint: None,
    };
    let mut in_comment = false;
    let (mut c_author, mut c_date, mut c_text) = (String::new(), String::new(), String::new());
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"Topic" => {
                    t.guid = attr(&e, b"Guid").unwrap_or_default();
                    if let Some(s) = attr(&e, b"TopicStatus") {
                        t.status = s;
                    }
                }
                b"Title" if !in_comment => t.title = text_of(&mut r),
                b"Description" if !in_comment => t.description = text_of(&mut r),
                b"Priority" => t.priority = text_of(&mut r),
                b"CreationAuthor" => t.author = text_of(&mut r),
                b"CreationDate" => t.created = text_of(&mut r),
                b"AssignedTo" => t.assignee = Some(text_of(&mut r)),
                b"Comment" if !in_comment => {
                    in_comment = true;
                    c_author.clear();
                    c_date.clear();
                    c_text.clear();
                }
                b"Comment" => c_text = text_of(&mut r),
                b"Author" if in_comment => c_author = text_of(&mut r),
                b"Date" if in_comment => c_date = text_of(&mut r),
                _ => {}
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"Comment" && in_comment => {
                in_comment = false;
                t.comments.push((c_author.clone(), c_date.clone(), c_text.clone()));
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (!t.title.is_empty()).then_some(t)
}

fn parse_viewpoint(xml: &str) -> Value {
    let mut r = Reader::from_str(xml);
    r.config_mut().trim_text(true);
    let mut selected = Vec::new();
    let mut cam: std::collections::HashMap<&'static str, [f64; 3]> = Default::default();
    let mut current: Option<&'static str> = None;
    let mut axis = 0usize;
    let mut point = [0.0f64; 3];
    let mut fov = 45.0;
    loop {
        match r.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.name().as_ref() == b"Component" => {
                if let Some(g) = attr(&e, b"IfcGuid") {
                    selected.push(g);
                }
            }
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"CameraViewPoint" => current = Some("position"),
                b"CameraDirection" => current = Some("direction"),
                b"CameraUpVector" => current = Some("up"),
                b"X" | b"Y" | b"Z" if current.is_some() => {
                    axis = match e.name().as_ref() {
                        b"X" => 0,
                        b"Y" => 1,
                        _ => 2,
                    };
                    point[axis] = text_of(&mut r).parse().unwrap_or(0.0);
                    if axis == 2 {
                        cam.insert(current.unwrap(), point);
                    }
                }
                b"FieldOfView" => fov = text_of(&mut r).parse().unwrap_or(45.0),
                _ => {}
            },
            Ok(Event::End(e)) if matches!(e.name().as_ref(), b"CameraViewPoint" | b"CameraDirection" | b"CameraUpVector") => current = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let _ = axis;
    let mut vp = json!({"selected": selected});
    if let (Some(p), Some(d), Some(u)) = (cam.get("position"), cam.get("direction"), cam.get("up")) {
        vp["camera"] = json!({"position": from_ifc(*p), "direction": from_ifc(*d), "up": from_ifc(*u), "fov": fov});
    }
    vp
}

/// Topics from a .bcfzip (BCF 2.1 or 3.0). Entries larger than `max_entry` bytes are skipped.
pub fn import(bytes: &[u8], max_entry: u64) -> anyhow::Result<Vec<Topic>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    let mut markups: Vec<(String, String)> = Vec::new();
    let mut viewpoints: std::collections::HashMap<String, String> = Default::default();
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        if f.size() > max_entry || f.is_dir() {
            continue;
        }
        let name = f.name().to_string();
        let mut s = String::new();
        if f.read_to_string(&mut s).is_err() {
            continue;
        }
        let dir = name.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        if name.ends_with("markup.bcf") {
            markups.push((dir, s));
        } else if name.ends_with(".bcfv") {
            viewpoints.entry(dir).or_insert(s);
        }
    }
    let mut out = Vec::new();
    for (dir, xml) in markups {
        if let Some(mut t) = parse_markup(&xml) {
            t.viewpoint = viewpoints.get(&dir).map(|v| parse_viewpoint(v));
            out.push(t);
        }
    }
    Ok(out)
}
