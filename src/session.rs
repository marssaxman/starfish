//! Append-only local exploration records. Source and previews are published together;
//! preferences are separate immutable events so corrections retain their history.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    program::ParameterBindings,
    render::{self, RenderRequest, RenderedImage},
    variation::{self, Settings, Snapshot},
};

pub const PREVIEW_SIZE: usize = 192;
const MARKER: &str = "Starfish exploration session 1\n";
static COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct Session {
    pub root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: String,
    pub parent: Option<String>,
    pub seed: Option<u64>,
    pub output: String,
}

#[derive(Clone, Debug)]
pub struct Round {
    pub id: String,
    pub context: String,
    pub parent: Option<String>,
    pub parents: Vec<String>,
    pub seed: Option<u64>,
    pub settings: Option<Settings>,
    pub pairs: Vec<[String; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    A,
    B,
    Both,
    Neither,
}

impl Preference {
    pub const ALL: [Self; 4] = [Self::A, Self::B, Self::Both, Self::Neither];
    pub fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::Both => "Both",
            Self::Neither => "Neither",
        }
    }
    fn parse(value: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|choice| choice.label() == value)
            .ok_or_else(|| "Invalid saved preference".into())
    }
}

#[derive(Default)]
pub struct Feedback {
    pub kept: HashSet<String>,
    pub choices: HashMap<(String, usize), Preference>,
    pub last: Option<String>,
    pub last_round: Option<String>,
}

impl Session {
    /// Open explicitly, or initialize an empty destination. Never claim an unrelated folder.
    pub fn open(root: &Path) -> Result<Self, String> {
        if !root.exists() {
            fs::create_dir_all(root).map_err(io_error)?;
        }
        let marker = root.join("session.txt");
        if !marker.exists() {
            if fs::read_dir(root).map_err(io_error)?.next().is_some() {
                return Err(format!(
                    "{} is not a Starfish exploration session",
                    root.display()
                ));
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)
                .map_err(io_error)?;
            file.write_all(MARKER.as_bytes()).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
        }
        if fs::read_to_string(&marker).map_err(io_error)? != MARKER {
            return Err("Unsupported exploration session version".into());
        }
        Ok(Self {
            root: root.to_owned(),
        })
    }

    pub fn candidates(&self) -> Result<Vec<Candidate>, String> {
        self.ids('c')?
            .into_iter()
            .map(|id| self.candidate(&id))
            .collect()
    }

    pub fn candidate(&self, id: &str) -> Result<Candidate, String> {
        let fields = fields(
            &self.path(id, 'c')?.join("meta.tsv"),
            "starfish-candidate\t1",
        )?;
        let parent = optional_id(get(&fields, "parent")?, 'c')?;
        Ok(Candidate {
            id: id.to_owned(),
            parent,
            seed: optional_number(get(&fields, "seed")?)?,
            output: get(&fields, "output")?.to_owned(),
        })
    }

    pub fn snapshot(&self, id: &str) -> Result<Snapshot, String> {
        let record = self.candidate(id)?;
        let snapshot = Snapshot {
            source: fs::read_to_string(self.path(id, 'c')?.join("program.sf")).map_err(io_error)?,
            output: record.output,
        };
        snapshot.program()?;
        Ok(snapshot)
    }

    pub fn save(&self, snapshot: &Snapshot, parent: Option<&str>) -> Result<Candidate, String> {
        if let Some(parent) = parent {
            self.candidate(parent)?;
        }
        let image = preview(snapshot)?;
        self.save_prepared(snapshot, parent, None, &image)
    }

    fn save_prepared(
        &self,
        snapshot: &Snapshot,
        parent: Option<&str>,
        seed: Option<u64>,
        image: &RenderedImage,
    ) -> Result<Candidate, String> {
        let id = unique('c');
        let metadata = format!(
            "starfish-candidate\t1\nparent\t{}\nseed\t{}\noutput\t{}\npolicy\t{}\n",
            parent.unwrap_or("-"),
            seed.map(|n| n.to_string()).unwrap_or("-".into()),
            snapshot.output,
            if seed.is_some() {
                variation::VERSION
            } else {
                "snapshot-v1"
            }
        );
        self.publish(&id, |directory| {
            write_sync(&directory.join("program.sf"), &snapshot.source)?;
            let mut file = File::create(directory.join("preview.ppm")).map_err(io_error)?;
            render::write_ppm(image, &mut file).map_err(io_error)?;
            file.sync_all().map_err(io_error)?;
            write_sync(&directory.join("meta.tsv"), &metadata)
        })?;
        self.candidate(&id)
    }

    pub fn keep(&self, id: &str) -> Result<(), String> {
        self.candidate(id)?;
        self.event(&format!("kind\tkeep\ncandidate\t{id}\n"))
    }

    pub fn visit(&self, id: &str) -> Result<(), String> {
        self.candidate(id)?;
        self.event(&format!("kind\tvisit\ncandidate\t{id}\n"))
    }

    pub fn view_round(&self, id: &str) -> Result<(), String> {
        self.round(id)?;
        self.event(&format!("kind\tview-round\nround\t{id}\n"))
    }

    pub fn image(&self, id: &str) -> Result<RenderedImage, String> {
        let bytes = fs::read(self.path(id, 'c')?.join("preview.ppm")).map_err(io_error)?;
        let header = format!("P6\n{PREVIEW_SIZE} {PREVIEW_SIZE}\n255\n");
        let rgb = bytes
            .strip_prefix(header.as_bytes())
            .ok_or("Invalid saved preview header")?;
        if rgb.len() != PREVIEW_SIZE * PREVIEW_SIZE * 3 {
            return Err("Incomplete saved preview".into());
        }
        let rgba = rgb
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect();
        Ok(RenderedImage {
            width: PREVIEW_SIZE,
            height: PREVIEW_SIZE,
            rgba,
            duration: std::time::Duration::ZERO,
        })
    }

    pub fn choose(&self, round: &Round, pair: usize, preference: Preference) -> Result<(), String> {
        let stored = self.round(&round.id)?;
        if pair >= stored.pairs.len() || stored.pairs != round.pairs {
            return Err("Preference does not identify a saved comparison".into());
        }
        self.event(&format!(
            "kind\tchoice\nround\t{}\npair\t{pair}\nchoice\t{}\n",
            round.id,
            preference.label()
        ))
    }

    pub fn feedback(&self) -> Result<Feedback, String> {
        let mut feedback = Feedback::default();
        for id in self.ids('e')? {
            let fields = fields(&self.path(&id, 'e')?.join("meta.tsv"), "starfish-event\t1")?;
            match get(&fields, "kind")? {
                kind @ ("keep" | "visit") => {
                    let id = get(&fields, "candidate")?;
                    self.candidate(id)?;
                    if kind == "keep" {
                        feedback.kept.insert(id.to_owned());
                    }
                    feedback.last = Some(id.to_owned());
                    feedback.last_round = None;
                }
                "view-round" => {
                    let id = get(&fields, "round")?;
                    self.round(id)?;
                    feedback.last_round = Some(id.to_owned());
                }
                "choice" => {
                    let id = get(&fields, "round")?;
                    let round = self.round(id)?;
                    let pair: usize = get(&fields, "pair")?
                        .parse()
                        .map_err(|_| "Invalid preference pair")?;
                    if pair >= round.pairs.len() {
                        return Err("Invalid preference pair".into());
                    }
                    feedback.choices.insert(
                        (id.to_owned(), pair),
                        Preference::parse(get(&fields, "choice")?)?,
                    );
                }
                _ => return Err("Unknown exploration event".into()),
            }
        }
        Ok(feedback)
    }

    pub fn rounds(&self) -> Result<Vec<Round>, String> {
        self.ids('r')?
            .into_iter()
            .map(|id| self.round(&id))
            .collect()
    }

    pub fn round(&self, id: &str) -> Result<Round, String> {
        let text = fs::read_to_string(self.path(id, 'r')?.join("meta.tsv")).map_err(io_error)?;
        let mut lines = text.lines();
        if lines.next() != Some("starfish-round\t1") {
            return Err("Unsupported comparison record".into());
        }
        let mut pairs = Vec::new();
        let mut metadata = HashMap::new();
        for line in lines {
            let columns: Vec<_> = line.split('\t').collect();
            match columns.as_slice() {
                ["pair", a, b] => {
                    self.candidate(a)?;
                    self.candidate(b)?;
                    pairs.push([a.to_string(), b.to_string()]);
                }
                [key, value] if *key != "pair" && !metadata.contains_key(*key) => {
                    metadata.insert(key.to_string(), value.to_string());
                }
                _ => return Err("Malformed comparison record".into()),
            }
        }
        if pairs.is_empty() {
            return Err("Empty comparison record".into());
        }
        let context = get(&metadata, "context")?.to_owned();
        let parent = optional_id(get(&metadata, "parent")?, 'c')?;
        let parents = match metadata.get("parents").map(String::as_str) {
            Some("-") => Vec::new(),
            Some(value) => value
                .split(',')
                .map(|id| self.candidate(id).map(|record| record.id))
                .collect::<Result<_, _>>()?,
            None => parent.iter().cloned().collect(),
        };
        let seed = optional_number(get(&metadata, "seed")?)?;
        let settings = match get(&metadata, "groups")? {
            "-" => None,
            value => Some(Settings::from_mask(
                value.parse().map_err(|_| "Invalid variation groups")?,
                get(&metadata, "strength")?
                    .parse()
                    .map_err(|_| "Invalid variation amount")?,
            )?),
        };
        Ok(Round {
            id: id.to_owned(),
            context,
            parent,
            parents,
            seed,
            settings,
            pairs,
        })
    }

    /// Both review candidates are captured before a vote, independent of mutable input files.
    pub fn review(&self, context: &str, snapshots: &[Snapshot; 2]) -> Result<Round, String> {
        if !context
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("Invalid comparison context".into());
        }
        if let Some(round) = self
            .rounds()?
            .into_iter()
            .find(|round| round.context == context)
        {
            return Ok(round);
        }
        let images = [preview(&snapshots[0])?, preview(&snapshots[1])?];
        let a = self.save_prepared(&snapshots[0], None, None, &images[0])?;
        let b = self.save_prepared(&snapshots[1], None, None, &images[1])?;
        self.write_round(context, &[], None, None, vec![[a.id, b.id]])
    }

    /// Prepare the entire batch before publishing children. Rejected/duplicate images
    /// consume deterministic attempts and never silently turn into identical A/B pairs.
    pub fn vary(
        &self,
        parent: &str,
        seed: u64,
        count: usize,
        settings: Settings,
    ) -> Result<Round, String> {
        self.vary_from(&[parent.to_owned()], seed, count, settings, "variations")
    }

    pub fn vary_picks(
        &self,
        round: &str,
        seed: u64,
        count: usize,
        settings: Settings,
    ) -> Result<Round, String> {
        let round = self.round(round)?;
        let feedback = self.feedback()?;
        let mut parents = Vec::new();
        for (index, pair) in round.pairs.iter().enumerate() {
            let selected: &[usize] = match feedback.choices.get(&(round.id.clone(), index)) {
                Some(Preference::A) => &[0],
                Some(Preference::B) => &[1],
                Some(Preference::Both) => &[0, 1],
                _ => &[],
            };
            for &slot in selected {
                if !parents.contains(&pair[slot]) {
                    parents.push(pair[slot].clone());
                }
            }
        }
        if parents.is_empty() {
            return Err(
                "Choose A, B, or Both in at least one pair before exploring your picks".into(),
            );
        }
        self.vary_from(
            &parents,
            seed,
            count,
            settings,
            &format!("picks-{}", round.id),
        )
    }

    pub fn regenerate(
        &self,
        round: &str,
        seed: u64,
        count: usize,
        settings: Settings,
    ) -> Result<Round, String> {
        let round = self.round(round)?;
        let parents = if round.parents.is_empty() {
            // Imported study comparisons have no shared parent. Start local
            // variations from their saved images rather than consulting votes.
            round.pairs.iter().flatten().cloned().collect()
        } else {
            round.parents
        };
        self.vary_from(
            &parents,
            seed,
            count,
            settings,
            &format!("regenerate-{}", round.id),
        )
    }

    fn vary_from(
        &self,
        parents: &[String],
        seed: u64,
        count: usize,
        settings: Settings,
        context: &str,
    ) -> Result<Round, String> {
        if !(1..=12).contains(&count) {
            return Err("Choose between 1 and 12 comparison pairs".into());
        }
        Settings::from_mask(settings.mask(), settings.strength)?;
        let snapshots = parents
            .iter()
            .map(|id| self.snapshot(id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut prepared = Vec::new();
        let mut sources = HashSet::new();
        let mut pixels = HashSet::new();
        for snapshot in &snapshots {
            pixels.insert(preview(snapshot)?.rgba);
        }
        for index in 0..count * 2 {
            // Siblings share a parent; pairs cycle through the selected families.
            // Rotate the starting family by seed when the pool exceeds the pair count.
            let parent_index = (index / 2 + (seed % parents.len() as u64) as usize) % parents.len();
            let snapshot = &snapshots[parent_index];
            let mut found = None;
            for attempt in 0..24u64 {
                let proposal_seed = seed
                    .wrapping_add((index as u64).wrapping_mul(0x9e3779b97f4a7c15))
                    .wrapping_add(attempt.wrapping_mul(0xd1b54a32d192ed03));
                let Ok(child) = variation::propose(snapshot, settings, proposal_seed) else {
                    continue;
                };
                if child.source == snapshot.source || sources.contains(&child.source) {
                    continue;
                }
                let Ok(image) = preview(&child) else {
                    continue;
                };
                if pixels.contains(&image.rgba) {
                    continue;
                }
                sources.insert(child.source.clone());
                pixels.insert(image.rgba.clone());
                found = Some((child, proposal_seed, image, parents[parent_index].clone()));
                break;
            }
            prepared.push(found.ok_or("These settings did not produce enough distinct images. Enable another feature or increase the variation amount.")?);
        }
        let mut ids = Vec::new();
        for (child, seed, image, parent) in prepared {
            ids.push(
                self.save_prepared(&child, Some(&parent), Some(seed), &image)?
                    .id,
            );
        }
        let pairs = ids
            .chunks_exact(2)
            .map(|pair| [pair[0].clone(), pair[1].clone()])
            .collect();
        self.write_round(context, parents, Some(seed), Some(settings), pairs)
    }

    fn write_round(
        &self,
        context: &str,
        parents: &[String],
        seed: Option<u64>,
        settings: Option<Settings>,
        pairs: Vec<[String; 2]>,
    ) -> Result<Round, String> {
        let id = unique('r');
        let mut metadata = format!(
            "starfish-round\t1\ncontext\t{context}\nparent\t{}\nseed\t{}\ngroups\t{}\nstrength\t{}\npolicy\t{}\n",
            if parents.len() == 1 {
                parents[0].as_str()
            } else {
                "-"
            },
            seed.map(|n| n.to_string()).unwrap_or("-".into()),
            settings.map(|s| s.mask().to_string()).unwrap_or("-".into()),
            settings
                .map(|s| s.strength.to_string())
                .unwrap_or("-".into()),
            variation::VERSION
        );
        metadata.push_str(&format!(
            "parents\t{}\n",
            if parents.is_empty() {
                "-".to_owned()
            } else {
                parents.join(",")
            }
        ));
        for [a, b] in pairs {
            metadata.push_str(&format!("pair\t{a}\t{b}\n"));
        }
        self.publish(&id, |directory| {
            write_sync(&directory.join("meta.tsv"), &metadata)
        })?;
        self.round(&id)
    }

    fn event(&self, fields: &str) -> Result<(), String> {
        self.publish(&unique('e'), |directory| {
            write_sync(
                &directory.join("meta.tsv"),
                &format!("starfish-event\t1\n{fields}"),
            )
        })
    }

    fn path(&self, id: &str, prefix: char) -> Result<PathBuf, String> {
        if !valid_id(id, prefix) {
            return Err("Invalid exploration record ID".into());
        }
        Ok(self.root.join(id))
    }

    fn ids(&self, prefix: char) -> Result<Vec<String>, String> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if valid_id(&name, prefix) && entry.file_type().map_err(io_error)?.is_dir() {
                ids.push(name);
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn publish(
        &self,
        id: &str,
        write: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let pending = self.root.join(format!(".pending-{id}"));
        let destination = self.root.join(id);
        fs::create_dir(&pending).map_err(io_error)?;
        let result = write(&pending).and_then(|_| {
            File::open(&pending)
                .and_then(|directory| directory.sync_all())
                .map_err(io_error)?;
            if destination.exists() {
                return Err("Exploration record already exists".into());
            }
            fs::rename(&pending, &destination).map_err(io_error)?;
            File::open(&self.root)
                .and_then(|directory| directory.sync_all())
                .map_err(io_error)
        });
        if result.is_err() {
            let _ = fs::remove_dir_all(&pending);
        }
        result
    }
}

pub fn preview(snapshot: &Snapshot) -> Result<RenderedImage, String> {
    let program = snapshot.program()?;
    render::render(
        &program,
        &ParameterBindings::defaults(&program),
        RenderRequest {
            output: &snapshot.output,
            width: PREVIEW_SIZE,
            height: PREVIEW_SIZE,
        },
    )
}

/// Stable across platforms; identity only, never used as a security boundary.
pub fn review_context(number: usize, snapshots: &[Snapshot; 2]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for snapshot in snapshots {
        for byte in snapshot
            .source
            .bytes()
            .chain([0])
            .chain(snapshot.output.bytes())
            .chain([0])
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("study-{number}-{hash:016x}")
}

fn unique(prefix: char) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{prefix}-{nanos:032x}-{:08x}-{:016x}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn valid_id(id: &str, prefix: char) -> bool {
    id.starts_with(&format!("{prefix}-"))
        && id.len() == 60
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-' || b == prefix as u8)
}

fn io_error(error: std::io::Error) -> String {
    format!("Cannot update exploration session: {error}")
}
fn optional_id(value: &str, prefix: char) -> Result<Option<String>, String> {
    if value == "-" {
        Ok(None)
    } else if valid_id(value, prefix) {
        Ok(Some(value.to_owned()))
    } else {
        Err("Invalid parent record ID".into())
    }
}
fn optional_number(value: &str) -> Result<Option<u64>, String> {
    if value == "-" {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|_| "Invalid saved seed".into())
    }
}
fn get<'a>(fields: &'a HashMap<String, String>, key: &str) -> Result<&'a str, String> {
    fields
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("Missing exploration field: {key}"))
}
fn fields(path: &Path, header: &str) -> Result<HashMap<String, String>, String> {
    let text = fs::read_to_string(path).map_err(io_error)?;
    let mut lines = text.lines();
    if lines.next() != Some(header) {
        return Err(format!("Unsupported record: {}", path.display()));
    }
    let mut fields = HashMap::new();
    for line in lines {
        let (key, value) = line
            .split_once('\t')
            .ok_or("Malformed exploration record")?;
        if value.contains('\t') || fields.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err("Malformed exploration record".into());
        }
    }
    Ok(fields)
}
fn write_sync(path: &Path, value: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(io_error)?;
    file.write_all(value.as_bytes()).map_err(io_error)?;
    file.sync_all().map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::StarfishProgram;

    fn parent() -> Snapshot {
        let program =
            StarfishProgram::from_source(include_str!("../programs/curve_textures.sf")).unwrap();
        let mut bindings = ParameterBindings::defaults(&program);
        bindings.0[1] = 3.25;
        Snapshot::capture(&program, &bindings, &program.output).unwrap()
    }

    #[test]
    fn sessions_reopen_edits_votes_corrections_and_branch_history() {
        let root = std::env::temp_dir().join(format!("starfish-session-{}", unique('c')));
        let store = Session::open(&root).unwrap();
        let source = parent();
        let original = store.save(&source, None).unwrap();
        store.keep(&original.id).unwrap();
        let round = store
            .vary(&original.id, 42, 1, Settings::default())
            .unwrap();
        assert_eq!(round.parent.as_deref(), Some(original.id.as_str()));
        for choice in Preference::ALL {
            store.choose(&round, 0, choice).unwrap();
        }
        let reopened = Session::open(&root).unwrap();
        assert_eq!(reopened.snapshot(&original.id).unwrap(), source);
        assert_eq!(
            reopened.image(&original.id).unwrap().rgba,
            preview(&source).unwrap().rgba
        );
        let feedback = reopened.feedback().unwrap();
        assert!(feedback.kept.contains(&original.id));
        assert_eq!(
            feedback.choices.get(&(round.id.clone(), 0)),
            Some(&Preference::Neither)
        );
        assert_eq!(reopened.ids('e').unwrap().len(), 5);
        let repeated = store
            .vary(&original.id, 42, 1, Settings::default())
            .unwrap();
        for (first, second) in round.pairs[0].iter().zip(&repeated.pairs[0]) {
            assert_eq!(
                store.snapshot(first).unwrap(),
                store.snapshot(second).unwrap()
            );
            assert_eq!(
                store.image(first).unwrap().rgba,
                store.image(second).unwrap().rgba
            );
            let record = store.candidate(first).unwrap();
            assert_eq!(record.parent.as_deref(), Some(original.id.as_str()));
            assert!(record.seed.is_some());
            assert_eq!(
                variation::propose(&source, Settings::default(), record.seed.unwrap()).unwrap(),
                store.snapshot(first).unwrap()
            );
        }
        let child = &round.pairs[0][0];
        let next = store
            .vary(
                child,
                99,
                1,
                Settings {
                    texture: false,
                    palette: true,
                    ..Settings::default()
                },
            )
            .unwrap();
        assert_eq!(next.parent.as_deref(), Some(child.as_str()));
        assert_ne!(
            preview(&store.snapshot(&round.pairs[0][0]).unwrap())
                .unwrap()
                .rgba,
            preview(&store.snapshot(&round.pairs[0][1]).unwrap())
                .unwrap()
                .rgba
        );
        assert!(store.choose(&round, 1, Preference::A).is_err());
        assert!(store.snapshot("../outside").is_err());
        let before = store.candidates().unwrap().len();
        assert!(
            store
                .vary(&original.id, 42, 0, Settings::default())
                .is_err()
        );
        assert!(
            store
                .vary(
                    &original.id,
                    42,
                    1,
                    Settings {
                        texture: false,
                        ..Settings::default()
                    }
                )
                .is_err()
        );
        assert_eq!(store.candidates().unwrap().len(), before);
        fs::create_dir(root.join(".pending-interrupted")).unwrap();
        assert_eq!(store.candidates().unwrap().len(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn review_votes_reference_immutable_programs_and_unrelated_folders_are_refused() {
        let root = std::env::temp_dir().join(format!("starfish-review-{}", unique('c')));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("important.txt"), "keep me").unwrap();
        assert!(Session::open(&root).is_err());
        assert_eq!(
            fs::read_to_string(root.join("important.txt")).unwrap(),
            "keep me"
        );
        let store = Session::open(&root.join("session")).unwrap();
        let first = parent();
        let second = variation::propose(&first, Settings::default(), 45).unwrap();
        let snapshots = [first, second];
        let context = review_context(1, &snapshots);
        let round = store.review(&context, &snapshots).unwrap();
        assert_eq!(round.id, store.review(&context, &snapshots).unwrap().id);
        store.choose(&round, 0, Preference::Both).unwrap();
        assert_eq!(store.snapshot(&round.pairs[0][0]).unwrap(), snapshots[0]);
        assert_eq!(
            Session::open(&store.root)
                .unwrap()
                .feedback()
                .unwrap()
                .choices
                .get(&(round.id, 0)),
            Some(&Preference::Both)
        );
        let review = store.rounds().unwrap().pop().unwrap();
        let regenerated = store
            .regenerate(&review.id, 54, 1, Settings::default())
            .unwrap();
        assert_eq!(regenerated.parents, review.pairs[0]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn picks_drive_the_next_round_and_neither_creates_no_descendants() {
        let root = std::env::temp_dir().join(format!("starfish-picks-{}", unique('c')));
        let store = Session::open(&root).unwrap();
        let parent = store.save(&parent(), None).unwrap();
        let round = store.vary(&parent.id, 51, 1, Settings::default()).unwrap();
        assert!(
            store
                .vary_picks(&round.id, 52, 1, Settings::default())
                .is_err()
        );
        for (choice, slots) in [
            (Preference::A, vec![0]),
            (Preference::B, vec![1]),
            (Preference::Both, vec![0, 1]),
        ] {
            store.choose(&round, 0, choice).unwrap();
            let next = store
                .vary_picks(&round.id, 52, 2, Settings::default())
                .unwrap();
            let expected: Vec<_> = slots
                .iter()
                .map(|&slot| round.pairs[0][slot].clone())
                .collect();
            assert_eq!(next.parents, expected);
            assert_eq!(next.context, format!("picks-{}", round.id));
            let actual: HashSet<_> = next
                .pairs
                .iter()
                .flatten()
                .map(|id| store.candidate(id).unwrap().parent.unwrap())
                .collect();
            assert_eq!(actual, expected.into_iter().collect());
            for pair in &next.pairs {
                assert_eq!(
                    store.candidate(&pair[0]).unwrap().parent,
                    store.candidate(&pair[1]).unwrap().parent
                );
            }
            if choice == Preference::Both {
                store.choose(&next, 0, Preference::Neither).unwrap();
                let settings = Settings {
                    texture: false,
                    palette: true,
                    strength: 0.2,
                    ..Settings::default()
                };
                let regenerated = store.regenerate(&next.id, 53, 2, settings).unwrap();
                assert_eq!(regenerated.parents, next.parents);
                assert_eq!(regenerated.settings, Some(settings));
                assert_eq!(regenerated.seed, Some(53));
                assert_eq!(regenerated.pairs.len(), 2);
                assert_eq!(regenerated.context, format!("regenerate-{}", next.id));
            }
        }
        store.choose(&round, 0, Preference::Neither).unwrap();
        let before = store.candidates().unwrap().len();
        assert!(
            store
                .vary_picks(&round.id, 52, 1, Settings::default())
                .is_err()
        );
        assert_eq!(store.candidates().unwrap().len(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_only_variations_fail_without_publishing_partial_children() {
        let root = std::env::temp_dir().join(format!("starfish-duplicates-{}", unique('c')));
        let store = Session::open(&root).unwrap();
        let source = r#"starfish 1; program "Constant study" {
            param unused: Scalar = 0.5 in 0..1;
            field constant = 0 * dot(world, vec2(1, 0));
            palette ink = smooth [rgb(0, 0, 0), rgb(1, 1, 1)];
            output image = palette(ink, constant);
        }"#;
        let parent = store
            .save(
                &Snapshot {
                    source: source.into(),
                    output: "image".into(),
                },
                None,
            )
            .unwrap();
        assert!(store.vary(&parent.id, 10, 1, Settings::default()).is_err());
        assert_eq!(store.candidates().unwrap().len(), 1);
        assert!(store.rounds().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
