use std::{
    collections::HashMap,
    io::{Read, Seek},
    num::NonZeroU8,
};

use getset::Getters;

use crate::{
    low_level::{self, BasicGeometry},
    DmxAddress, Gdtf, GdtfError, GdtfParseError, Geometry, GeometryType, HandleProblem,
    IntoValidName, Name, PlaceGdtfError, Problems, ProblemsMut,
};

use super::low_level::ParsedGdtf;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct ValidatedGdtf {
    gdtf: Gdtf,
    problems: Problems,
}

impl ValidatedGdtf {
    pub fn from_reader<T: Read + Seek>(reader: T) -> Result<Self, GdtfParseError> {
        let low_level_parsed = ParsedGdtf::from_reader(reader)?;
        Ok(low_level_parsed.into())
    }
}

impl From<ParsedGdtf> for ValidatedGdtf {
    fn from(mut parsed: ParsedGdtf) -> Self {
        let mut gdtf = Gdtf::default();

        let rename_map = validate_geometries(&mut parsed, &mut gdtf);

        gdtf.data_version = parsed.gdtf.data_version;
        gdtf.name = parsed.gdtf.fixture_type.name;
        gdtf.short_name = parsed.gdtf.fixture_type.short_name;
        gdtf.long_name = parsed.gdtf.fixture_type.long_name;
        gdtf.manufacturer = parsed.gdtf.fixture_type.manufacturer;
        gdtf.description = parsed.gdtf.fixture_type.description;
        gdtf.fixture_type_id = parsed.gdtf.fixture_type.id;
        gdtf.ref_ft = parsed.gdtf.fixture_type.ref_ft;
        gdtf.can_have_children = bool::from(parsed.gdtf.fixture_type.can_have_children);

        ValidatedGdtf {
            gdtf,
            problems: parsed.problems,
        }
    }
}

/// maps (top level name, duplicate geometry name) => renamed name
#[derive(Default)]
pub(crate) struct GeometryLookup(HashMap<(Name, Name), Name>);

fn validate_geometries(parsed: &mut ParsedGdtf, gdtf: &mut Gdtf) -> GeometryLookup {
    let input = &parsed.gdtf.fixture_type.geometries.children;
    let p = &mut parsed.problems;

    let mut rename_map = GeometryLookup::default();

    let mut descendants = Vec::new();

    for g in input.iter() {
        if let Some((to_add, children)) = translate_geometry(g, p) {
            let parent_name = to_add.name.clone();
            gdtf.add_top_level_geometry(to_add)
                .map(|()| descendants.push((parent_name, children)))
                .at("top level geometries")
                .ok_or_handled_by("ignoring node", p);
        }
    }

    for (parent_name, children) in descendants {
        maybe_add_children_geometries(
            children,
            gdtf,
            &parent_name,
            &parent_name,
            &mut rename_map,
            p,
        );
    }

    rename_map
}

fn maybe_add_children_geometries(
    children: Option<Vec<low_level::GeometryType>>,
    gdtf: &mut Gdtf,
    parent: &Name,
    top_level: &Name,
    rename_map: &mut GeometryLookup,
    p: &mut impl ProblemsMut,
) {
    if let Some(children) = children {
        for c in children {
            validate_child_geometry_recursively(c, gdtf, parent, top_level, rename_map, p);
        }
    }
}

fn validate_child_geometry_recursively(
    g: low_level::GeometryType,
    gdtf: &mut Gdtf,
    parent: &Name,
    top_level: &Name,
    rename_map: &mut GeometryLookup,
    p: &mut impl ProblemsMut,
) -> Option<()> {
    let (to_add, children) = translate_geometry(&g, p)?;
    let new_name = to_add.name.clone();
    match gdtf.add_child_geometry(parent, to_add) {
        Ok(_) => {
            maybe_add_children_geometries(children, gdtf, &new_name, top_level, rename_map, p);
        }
        Err(GdtfError::DuplicateGeometryName(mut g)) => {
            // rename
            let mut e = GdtfError::DuplicateGeometryName(g.clone());
            let original_name = g.name.clone();
            let name_with_top_level = format!("{} (in {top_level})", g.name).into_valid();
            g.name = name_with_top_level.clone();
            (g, e) = try_inserting_renamed(
                g,
                gdtf,
                parent,
                top_level,
                &original_name,
                rename_map,
                p,
                e,
            )?;

            for dedup_ind in 1..10_000 {
                let name_with_counter =
                    format!("{name_with_top_level} (duplicate {dedup_ind})").into_valid();
                g.name = name_with_counter;
                (g, e) = try_inserting_renamed(
                    g,
                    gdtf,
                    parent,
                    top_level,
                    &original_name,
                    rename_map,
                    p,
                    e,
                )?;
            }
        }
        r => {
            r.at(format!("geometry '{new_name}'"))
                .ok_or_handled_by("ignore geometry and its possible children", p);
        }
    }
    Some(())
}

/// Returning None means success, returning Some means continue working on the geometry
fn try_inserting_renamed(
    g: Geometry,
    gdtf: &mut Gdtf,
    parent: &Name,
    top_level: &Name,
    original_name: &Name,
    rename_map: &mut GeometryLookup,
    p: &mut impl ProblemsMut,
    e: GdtfError,
) -> Option<(Geometry, GdtfError)> {
    let renamed = g.name.clone();
    match gdtf.add_child_geometry(parent, g) {
        Ok(_) => {
            let pe = e.at("Geometries");
            match rename_map
                .0
                .entry((top_level.clone(), original_name.clone()))
            {
                std::collections::hash_map::Entry::Occupied(_) => {
                    pe.handled_by(format!("renaming to '{renamed}', multiple geometries of this same name in its top level geometry will make references non-unique"), p);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    pe.handled_by(format!("renaming to '{renamed}'"), p);
                    entry.insert(renamed);
                }
            }
            None
        }
        Err(GdtfError::DuplicateGeometryName(g)) => Some((g, e)),
        Err(ue) => {
            e.at("Geometries")
                .handled_by("ignoring geometry because later error", p);
            ue.at("Geometries").handled_by("ignoring geometry", p);
            None
        }
    }
}

fn translate_geometry(
    g: &low_level::GeometryType,
    p: &mut impl ProblemsMut,
) -> Option<(Geometry, Option<Vec<low_level::GeometryType>>)> {
    match g {
        crate::low_level::GeometryType::Geometry { basic, children } => Some((
            Geometry {
                name: basic.name.clone(),
                t: GeometryType::Geometry {
                    children: Vec::new(),
                },
            },
            Some(children.clone()),
        )),
        crate::low_level::GeometryType::GeometryReference {
            basic,
            geometry,
            breaks,
        } => Some((translate_reference(basic, geometry, breaks, p)?, None)),
    }
}

fn translate_reference(
    basic: &BasicGeometry,
    geometry: &Name,
    breaks: &[low_level::Break],
    p: &mut impl ProblemsMut,
) -> Option<Geometry> {
    let mut offsets = HashMap::new();
    let mut break_iter = breaks.iter().rev(); // start at last break which sets default (i.e. overwrite)

    let overwrite = break_iter
        .next()
        .map(|b| (b.dmx_break, b.dmx_offset.clone()))
        .unwrap_or((NonZeroU8::MIN, DmxAddress::default()));

    break_iter.for_each(|b| match offsets.entry(b.dmx_break) {
        std::collections::hash_map::Entry::Occupied(entry) => {
            todo!("add to problems as duplicate, don't use it")
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(b.dmx_offset.clone());
        }
    });

    offsets.entry(overwrite.0).or_insert(overwrite.1.clone());

    Some(Geometry {
        name: basic.name.clone(),
        t: GeometryType::GeometryReference {
            geometry: geometry.clone(),
            overwrite,
            offsets,
        },
    })
}
