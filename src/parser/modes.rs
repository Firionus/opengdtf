use std::{
    cmp::{max, min},
    collections::HashMap,
};

use itertools::Itertools;
use petgraph::graph::NodeIndex;
use roxmltree::Node;
use thiserror::Error;

use crate::{
    channel_offsets::ChannelOffsets,
    dmx_break::Break,
    dmx_modes::{Channel, ChannelFunction, DmxMode, ModeMaster, Subfixture},
    geometries::Geometries,
    geometry::{Geometry, Type},
    name::{IntoValidName, Name},
    ParsedGdtf, Problem, ProblemAt, Problems,
};

use super::{
    dmx_value::{bytes_max_value, parse_dmx},
    parse_xml::{get_xml_attribute::parse_attribute_content, GetXmlAttribute, GetXmlNode},
    problems::{HandleOption, HandleProblem, ProblemsMut, TransformUnexpected},
};

// TODO First and foremost: Clean up this complete mess of code!
// - Everything should be scoped to a function that returns Result
// - Functions shouldn't have 10 args, instead use additional builders for mode/channel and impl on them
// - review naming: Abstract vs template, chf vs channel_function (I'm in favor of chf), etc.
// - split into maybe 2-3 files?

impl ParsedGdtf {
    pub(crate) fn parse_dmx_modes(&mut self, fixture_type: Node) {
        let modes = match fixture_type.find_required_child("DMXModes") {
            Ok(v) => v,
            Err(p) => {
                p.handled_by("leaving DMX modes empty", self);
                return;
            }
        };

        for (i, mode) in modes
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "DMXMode")
            .enumerate()
        {
            DmxModeParser::parse(mode, i, self).ok_or_handled_by("ignoring DMX Mode", self);
        }
    }
}

struct DmxModeParser<'a> {
    parsed: &'a mut ParsedGdtf,
    mode_master_queue: Vec<DeferredModeMaster<'a>>,
    template_channels: TemplateChannels,
    mode_ind: usize,
    mode_node: Node<'a, 'a>,
    mode_name: Name,
}

impl<'a> ProblemsMut for DmxModeParser<'a> {
    fn problems_mut(&mut self) -> &mut Problems {
        &mut self.parsed.problems
    }
}

impl<'a> DmxModeParser<'a> {
    fn geometries(&self) -> &Geometries {
        &self.parsed.gdtf.geometries
    }

    fn mode_mut(&mut self) -> Result<&mut DmxMode, ProblemAt> {
        self.parsed
            .gdtf
            .dmx_mode_mut(self.mode_ind)
            .map_err(|e| Problem::from(e).at(&self.mode_node))
    }

    fn mode(&self) -> Result<&DmxMode, ProblemAt> {
        self.parsed
            .gdtf
            .dmx_mode(self.mode_ind)
            .map_err(|e| Problem::from(e).at(&self.mode_node))
    }

    fn parse(mode_node: Node, i: usize, parsed: &'a mut ParsedGdtf) -> Result<(), ProblemAt> {
        let name = mode_node.name(i, parsed);
        let description = mode_node.attribute("Description").unwrap_or("").to_owned();

        let mode_geometry_name = mode_node.parse_required_attribute("Geometry")?;
        let geometry = parsed
            .gdtf
            .geometries
            .get_index(&mode_geometry_name)
            .ok_or_else(|| {
                Problem::UnknownGeometry(mode_geometry_name.to_owned()).at(&mode_node)
            })?;

        let mode_ind = parsed
            .gdtf
            .add_dmx_mode(name.clone(), description, geometry)
            .map_err(|e| Problem::from(e).at(&mode_node))?;

        let mut parser = DmxModeParser {
            parsed,
            mode_master_queue: Default::default(),
            template_channels: Default::default(),
            mode_ind,
            mode_node,
            mode_name: name,
        };

        mode_node
            .find_required_child("DMXChannels")
            .map(|n| parser.parse_dmx_channels(n))
            .ok_or_handled_by("leaving DMX mode empty", &mut parser);
        Ok(())
    }

    fn parse_dmx_channels<'b: 'a>(&mut self, dmx_channels: Node<'b, 'b>) {
        for channel in dmx_channels
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "DMXChannel")
        {
            self.parse_dmx_channel(channel)
                .ok_or_handled_by("ignoring channel", self);
        }

        while let Some(deferred_mode_master) = self.mode_master_queue.pop() {
            self.handle_mode_master(deferred_mode_master)
                .ok_or_handled_by("ignoring mode master", self);
        }
    }

    fn parse_dmx_channel<'b: 'a>(&mut self, channel: Node<'b, 'b>) -> Result<(), ProblemAt> {
        // TODO look up geometry in geometry rename lookup instead of geometries!
        let geometry_index = channel
            .parse_required_attribute("Geometry")
            .and_then(|geometry| {
                self.geometries()
                    .get_index(&geometry)
                    .ok_or_else(|| Problem::UnknownGeometry(geometry).at(&channel))
            })
            .ok_or_handled_by("using mode geometry", self)
            .unwrap_or(*self.mode_mut()?.geometry());

        // GDTF 1.2 says this field should be a "Node" (we call it NamePath)
        // But Attributes aren't nested, so there should only ever be one Name here, with no dot
        let first_logic_attribute: Name = channel
            .find_required_child("LogicalChannel")
            .and_then(|n| n.parse_required_attribute("Attribute"))
            .ok_or_handled_by("using empty", self)
            .unwrap_or_default();

        let name = {
            let geometry_name = &self
                .parsed
                .gdtf
                .geometries
                .get_by_index(geometry_index)
                .unexpected_err_at(&channel)?
                .name;
            format!("{geometry_name}_{first_logic_attribute}").into_valid()
        };

        let dmx_break = channel
            .attribute("DMXBreak")
            .and_then(|s| {
                match s {
                    "Overwrite" => Ok(ChannelBreak::Overwrite),
                    s => parse_attribute_content(&channel, s, "DMXBreak").map(ChannelBreak::Break),
                }
                .ok_or_handled_by("using default", self)
            })
            .unwrap_or_default();

        let mut offsets: ChannelOffsets = channel
            .parse_attribute("Offset")
            .transpose()
            .ok_or_handled_by("using None", self)
            .flatten()
            .unwrap_or_default();

        let channel_bytes = if offsets.is_empty() {
            4 // use maximum resolution for virtual channel
        } else if offsets.len() > 4 {
            Problem::UnsupportedByteCount(offsets.len())
                .at(&channel)
                .handled_by("using only 4 most significant bytes", self);
            offsets.truncate(4);
            4
        } else {
            offsets.len() as u8
        };
        let max_dmx_value = bytes_max_value(channel_bytes);

        let mut channel_functions = Vec::<(ChannelFunction, Node)>::default();
        // let mut channel_function_ids: Vec<NodeIndex> = Default::default();
        let raw_channel_function = ChannelFunction {
            name: name.to_owned(),
            geometry: geometry_index,
            attr: "NoFeature".into(),
            original_attr: "RawDMX".into(),
            dmx_from: 0,
            dmx_to: max_dmx_value,
            phys_from: 0.,
            phys_to: 1.,
            default: 0,
        };
        channel_functions.push((raw_channel_function, channel));

        for (_k, logical_channel) in channel
            .children()
            .filter(|n| n.has_tag_name("LogicalChannel"))
            .enumerate()
        {
            // TODO parse Snap, Master, MibFade, DMXChangeTimeLimit
            let mut chf_iter = logical_channel
                .children()
                .filter(|n| n.has_tag_name("ChannelFunction"))
                .enumerate()
                .peekable();

            while let Some((l, chf)) = chf_iter.next() {
                let channel_function = self.parse_channel_function(
                    chf,
                    l,
                    channel_bytes,
                    chf_iter.peek().map(|v| v.1),
                    max_dmx_value,
                    geometry_index,
                )?;
                channel_functions.push((channel_function, chf));
            }
        }

        let default = match channel
            .attribute("InitialFunction")
            .and_then(|s| {
                s.split('.')
                    .next_tuple()
                    .filter(|(ch, _lch, _chf)| (&name == ch))
                    .map(|(_ch, _lch, chf)| chf)
                    .ok_or_else(|| {
                        Problem::InvalidInitialFunction {
                            s: s.to_owned(),
                            channel: name.to_owned(),
                            mode: self.mode_name.to_owned(),
                        }
                        .at(&channel)
                    })
                    .ok_or_handled_by("using default", self)
            })
            .and_then(|chf_name| {
                channel_functions
                    .iter()
                    .find(|(chf, _)| chf.name == chf_name)
            })
            .map(|(chf, _)| chf.default)
        {
            Some(d) => d,
            None => {
                let mut it = channel_functions.iter();
                let raw = it.next();
                match it.next() {
                    Some(first) => first.0.default,
                    None => {
                        raw.ok_or_unexpected_at("no raw channel function", &channel)?
                            .0
                            .default
                    }
                }
            }
        };

        if !self.geometries().is_template(geometry_index) {
            let actual_dmx_break = match dmx_break {
                ChannelBreak::Break(b) => b,
                ChannelBreak::Overwrite => Err(Problem::InvalidBreakOverwrite {
                    ch: name.to_owned(),
                    mode: self.mode_name.to_owned(),
                }
                .at(&channel))
                .ok_or_handled_by("using break 1", self)
                .unwrap_or_default(),
            };

            let channel_function_ids =
                self.add_channel_functions(channel_functions, None, &name)?;

            let channel = Channel {
                name,
                dmx_break: actual_dmx_break,
                offsets,
                channel_functions: channel_function_ids,
                bytes: channel_bytes,
                default,
            };
            self.mode_mut()?.channels.push(channel);
        } else {
            // template channel
            let mut instances = HashMap::<Name, Name>::new(); // Subfixture Name -> Instantiated Channel Name
            let template_references: Vec<_> = self
                .geometries()
                .template_references(geometry_index)
                .collect();
            for ref_ind in template_references {
                let (reference_name, reference_offsets) = {
                    let reference = self
                        .geometries()
                        .get_by_index(ref_ind)
                        .unexpected_err_at(&channel)?;
                    let reference_offsets = if let Geometry {
                        name: _,
                        t: Type::Reference { offsets },
                    } = reference
                    {
                        offsets
                    } else {
                        Problem::Unexpected(
                            "template pointed to geometry that was not a reference".into(),
                        )
                        .at(&channel)
                        .handled_by("skipping", self);
                        continue;
                    };

                    (reference.name.clone(), reference_offsets.clone())
                };

                let (actual_dmx_break, offsets_offset) = match dmx_break {
                    ChannelBreak::Overwrite => match &reference_offsets.overwrite {
                        Some(o) => (o.dmx_break, o.offset),
                        None => {
                            Problem::MissingBreakInReference {
                                br: "Overwrite".into(),
                                ch: name.to_owned(),
                                mode: self.mode_name.to_owned(),
                            }
                            .at(&channel)
                            .handled_by("skipping", self);
                            continue;
                        }
                    },
                    ChannelBreak::Break(b) => (
                        b,
                        match reference_offsets.normal.get(&b) {
                            Some(o) => *o,
                            None => {
                                Problem::MissingBreakInReference {
                                    br: format!("{b}"),
                                    ch: name.to_owned(),
                                    mode: self.mode_name.to_owned(),
                                }
                                .at(&channel)
                                .handled_by("skipping", self);
                                continue;
                            }
                        },
                    ),
                };

                let channel_name = format!("{reference_name}_{first_logic_attribute}").into_valid();

                let channel_function_ids = self.add_channel_functions(
                    channel_functions.iter().enumerate().map(|(i, (chf, n))| {
                        let chf_name = if i == 0 {
                            channel_name.clone()
                        } else {
                            chf.name.clone()
                        };
                        (
                            ChannelFunction {
                                geometry: ref_ind, // TODO doesn't work with multi-level geometry reference,
                                // then the corresponding lower level instantiated geometry would be needed, which doesn't exist
                                name: chf_name,
                                ..chf.clone()
                            },
                            *n,
                        )
                    }),
                    Some(reference_name.to_owned()),
                    &name,
                )?;

                let dmx_channel = Channel {
                    name: channel_name,
                    dmx_break: actual_dmx_break,
                    offsets: offsets
                        .iter()
                        .map(|o| o + (offsets_offset as u16) - 1)
                        .collect(),
                    channel_functions: channel_function_ids,
                    bytes: channel_bytes,
                    default,
                };
                let sf: &mut Subfixture = if let Some(sf) = self
                    .mode_mut()?
                    .subfixtures
                    .iter_mut()
                    .find(|sf| sf.geometry == ref_ind)
                {
                    sf
                } else {
                    self.mode_mut()?.subfixtures.push(Subfixture {
                        name: reference_name.to_owned(),
                        channels: vec![],
                        geometry: ref_ind,
                    });
                    self.mode_mut()?
                        .subfixtures
                        .iter_mut()
                        .last()
                        .ok_or_unexpected_at("just pushed", &channel)?
                };

                if let Some(n) = instances.insert(sf.name.to_owned(), dmx_channel.name.to_owned()) {
                    Err(
                        Problem::Unexpected(format!("added subfixture {n} multiple times").into())
                            .at(&channel),
                    )?
                };
                sf.channels.push(dmx_channel);
            }
            if let Some(n) = self.template_channels.insert(name, instances) {
                Err(Problem::Unexpected(
                    format!("template channel name {n:?} encountered multiple times").into(),
                )
                .at(&channel))?
            };
        }

        Ok(())
    }

    fn add_channel_functions<'b: 'a>(
        &mut self,
        chfs: impl IntoIterator<Item = (ChannelFunction, Node<'b, 'b>)>,
        subfixture: Option<Name>,
        channel_name: &Name,
    ) -> Result<Vec<NodeIndex>, ProblemAt> {
        let mut channel_function_ids = Vec::<NodeIndex>::new();
        for (i, (chf, chf_node)) in chfs.into_iter().enumerate() {
            let chf_name = chf.name.to_owned();
            let chf_index = self
                .mode_mut()?
                .channel_functions
                .add_node(chf)
                .unexpected_err_at(&chf_node)?;
            channel_function_ids.push(chf_index);
            if i == 0 {
                // raw channel function has no ModeMaster
                continue;
            }
            if chf_node.attribute("ModeMaster").is_some() {
                self.mode_master_queue.push(DeferredModeMaster {
                    chf_node,
                    ch_name: channel_name.to_owned(),
                    chf_name,
                    chf_ind: chf_index,
                    subfixture: subfixture.to_owned(),
                });
            };
        }
        Ok(channel_function_ids)
    }

    fn parse_channel_function<'b>(
        &mut self,
        chf: Node<'b, 'b>,
        index_in_parent: usize,
        channel_bytes: u8,
        next_chf: Option<Node>,
        max_dmx_value: u32,
        geometry_index: NodeIndex,
    ) -> Result<ChannelFunction, ProblemAt> {
        let chf_attr = chf.attribute("Attribute").unwrap_or("NoFeature");
        let original_attribute = chf.attribute("OriginalAttribute").unwrap_or("");
        let chf_name: Name = chf
            .parse_attribute("Name")
            .and_then(|r| r.ok_or_handled_by("using default", self))
            .unwrap_or_else(|| format!("{chf_attr} {}", index_in_parent + 1).into_valid());
        let dmx_from = chf
            .attribute("DMXFrom")
            .and_then(|s| {
                parse_dmx(s, channel_bytes)
                    .map_err(|e| {
                        Problem::InvalidAttribute {
                            attr: "DMXFrom".to_owned(),
                            tag: "ChannelFunction".to_owned(),
                            content: s.to_owned(),
                            source: Box::new(e),
                            expected_type: "DMXValue".to_owned(),
                        }
                        .at(&chf)
                    })
                    .ok_or_handled_by("using default 0", self)
            })
            .unwrap_or(0);
        // The convention to use the next ChannelFunction in XML order for DMXTo is not official
        // but probably correct for GDTF Builder files.
        // see https://github.com/mvrdevelopment/spec/issues/103#issuecomment-985361192
        let dmx_to = next_chf
            .and_then(|next_chf| {
                let s = next_chf.attribute("DMXFrom").unwrap_or("0/1");
                parse_dmx(s, channel_bytes)
                    .map_err(|e| {
                        Problem::InvalidAttribute {
                            attr: "DMXFrom".to_owned(),
                            tag: "ChannelFunction".to_owned(),
                            content: s.to_owned(),
                            source: Box::new(e),
                            expected_type: "DMXValue".to_owned(),
                        }
                        .at(&chf)
                    })
                    .ok_or_handled_by(
                        "using maximum channel value for DMXTo of previous channel function",
                        self,
                    )
            })
            .filter(|next_dmx_from| dmx_from < *next_dmx_from)
            .map(|next_dmx_from| next_dmx_from - 1)
            .unwrap_or(max_dmx_value);
        let default = chf
            .attribute("Default")
            .and_then(|s| {
                parse_dmx(s, channel_bytes)
                    .map_err(|e| {
                        Problem::InvalidAttribute {
                            attr: "Default".to_owned(),
                            tag: "ChannelFunction".to_owned(),
                            content: s.to_owned(),
                            source: Box::new(e),
                            expected_type: "DMXValue".to_owned(),
                        }
                        .at(&chf)
                    })
                    .ok_or_handled_by("using default 0", self)
            })
            .unwrap_or(0);
        let phys_from = chf
            .parse_attribute("PhysicalFrom")
            .transpose()
            .ok_or_handled_by("using default 0", self)
            .flatten()
            .unwrap_or(0.);
        let phys_to = chf
            .parse_attribute("PhysicalTo")
            .transpose()
            .ok_or_handled_by("using default 1", self)
            .flatten()
            .unwrap_or(1.);

        Ok(ChannelFunction {
            name: chf_name,
            geometry: geometry_index,
            attr: chf_attr.to_owned(),
            original_attr: original_attribute.to_owned(),
            dmx_from,
            dmx_to,
            phys_from,
            phys_to,
            default,
        })
    }

    fn handle_mode_master(&mut self, d: DeferredModeMaster) -> Result<(), ProblemAt> {
        let mode_master = d
            .chf_node
            .attribute("ModeMaster")
            .ok_or_unexpected_at("mode master expected", &d.chf_node)?;

        let mut master_path = mode_master.split('.');
        let master_channel_name: Name = master_path
            .next()
            .ok_or_unexpected_at(
                "string splits always have at least one element",
                &d.chf_node,
            )?
            .into_valid();

        let (dependency_chfs, dependency_bytes) = {
            // TODO this clone is really only there to get out of borrowchecker hell
            let master_lookup = self.template_channels.get(&master_channel_name).cloned();
            let dependency_channel: &Channel = match master_lookup {
                Some(hm) => {
                    // master is template
                    let subfixture = d.subfixture.ok_or_else(|| {
                        Problem::AmbiguousModeMaster {
                            mode_master: master_channel_name.to_owned(),
                            ch: d.ch_name.to_owned(),
                            mode: self.mode_name.to_owned(),
                        }
                        .at(&d.chf_node)
                    })?;
                    let instantiated_master_name = hm.get(&subfixture).ok_or_else(|| {
                        Problem::AmbiguousModeMaster {
                            mode_master: master_channel_name.to_owned(),
                            ch: d.ch_name.to_owned(),
                            mode: self.mode_name.to_owned(),
                        }
                        .at(&d.chf_node)
                    })?;
                    self.mode_mut()?
                        .subfixtures
                        .iter()
                        .find(|sf| sf.name == subfixture)
                        .and_then(|sf| {
                            sf.channels
                                .iter()
                                .find(|ch| ch.name == *instantiated_master_name)
                        })
                        .ok_or_unexpected_at("subfixtures not present", &d.chf_node)?
                }
                None => {
                    // TODO this clone is only here because of borrowchecker hell
                    let mode_name = self.mode_name.clone();
                    // master is not a template
                    self.mode_mut()?
                        .channels
                        .iter()
                        .find(|ch| ch.name == master_channel_name)
                        .ok_or_else(|| {
                            Problem::UnknownChannel(master_channel_name, mode_name).at(&d.chf_node)
                        })?
                }
            };
            (
                dependency_channel.channel_functions.clone(),
                dependency_channel.bytes,
            )
        };
        // TODO how does this interact with renamed geometries? Wouldn't the channel then also have a different name? -> Geometry Renaming was a mistake, let's face it...
        // Geometry Lookup always works inside a specific top-level geometry, only inside that the names need to be unique
        // requiring more is taking the spec at face value and not following the conventions, which is a mistake with GDTF if one wants to be productive

        let (master, master_index): (&ChannelFunction, NodeIndex) = if master_path.next().is_some()
        {
            // reference to channel function
            let dependency_chf_name = master_path.next().ok_or_else(|| {
                Problem::InvalidAttribute {
                    attr: "ModeMaster".into(),
                    tag: "ChannelFunction".into(),
                    content: mode_master.into(),
                    source: ModeMasterParseError {}.into(),
                    expected_type: "Node".into(),
                }
                .at(&d.chf_node)
            })?;
            let mut master_chf = Default::default();
            // TODO replace with custom method on Channel
            for ni in dependency_chfs.iter() {
                let chf_candidate =
                    self.mode()?
                        .channel_functions
                        .node_weight(*ni)
                        .ok_or_else(|| {
                            Problem::Unexpected("Invalid Channel Function Index".into())
                                .at(&d.chf_node)
                        })?;
                if chf_candidate.name == dependency_chf_name {
                    master_chf = Some((chf_candidate, *ni));
                    break;
                }
            }
            master_chf.ok_or_else(|| {
                Problem::UnknownChannelFunction {
                    name: dependency_chf_name.into_valid(),
                    mode: self.mode_name.clone(),
                }
                .at(&d.chf_node)
            })?
        } else {
            let mode = self.mode_mut()?;
            // reference to channel, so in our interpretation to the raw dmx channel function
            dependency_chfs
                .get(0)
                .and_then(|i| mode.channel_functions.node_weight(*i).map(|chf| (chf, *i)))
                .ok_or_else(|| {
                    Problem::Unexpected("no raw dmx channel function".into()).at(&d.chf_node)
                })?
        };
        let master_from = master.dmx_from;
        let master_to = master.dmx_to;

        let (mode_from, mode_to) = if let (Some(mode_from), Some(mode_to)) = (
            d.chf_node.attribute("ModeFrom"),
            d.chf_node.attribute("ModeTo"),
        ) {
            (mode_from, mode_to)
        } else {
            Err(Problem::MissingModeFromOrTo(d.chf_name.to_string()).at(&d.chf_node))?
        };

        let mode_from = parse_dmx(mode_from, dependency_bytes)
            .map_err(|e| {
                Problem::InvalidAttribute {
                    attr: "ModeFrom".into(),
                    tag: "ChannelFunction".into(),
                    content: mode_from.into(),
                    source: Box::new(e),
                    expected_type: "DMXValue".into(),
                }
                .at(&d.chf_node)
            })
            .ok_or_handled_by("using default 0", self)
            .unwrap_or(0);
        let mode_to = parse_dmx(mode_to, dependency_bytes)
            .map_err(|e| {
                Problem::InvalidAttribute {
                    attr: "ModeTo".into(),
                    tag: "ChannelFunction".into(),
                    content: mode_to.into(),
                    source: Box::new(e),
                    expected_type: "DMXValue".into(),
                }
                .at(&d.chf_node)
            })
            .ok_or_handled_by("using default 0", self)
            .unwrap_or(0);

        let clipped_mode_from = max(mode_from, master_from);
        let clipped_mode_to = min(mode_to, master_to);

        if clipped_mode_to < clipped_mode_from {
            return Err(Problem::UnreachableChannelFunction {
                name: d.chf_name,
                dmx_mode: self.mode_name.to_owned(),
                mode_from,
                mode_to,
            }
            .at(&d.chf_node));
        }

        self.mode_mut()?
            .channel_functions
            .add_edge(
                master_index,
                d.chf_ind,
                ModeMaster {
                    from: clipped_mode_from,
                    to: clipped_mode_to,
                },
            )
            .unexpected_err_at(&d.chf_node)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("mode master attribute must contain either zero or two period separators")]
pub struct ModeMasterParseError();

#[derive(Debug, Clone)]
pub enum ChannelBreak {
    Overwrite,
    Break(Break),
}

impl Default for ChannelBreak {
    fn default() -> Self {
        ChannelBreak::Break(Break::default())
    }
}

/// Remember the relationship between original name and instance names of template channels
/// Structure: Original Channel Name -> Subfixture Name -> Instantiated Channel Name
type TemplateChannels = HashMap<Name, HashMap<Name, Name>>;

struct DeferredModeMaster<'a> {
    chf_node: Node<'a, 'a>,
    ch_name: Name,
    chf_name: Name,
    chf_ind: NodeIndex,
    subfixture: Option<Name>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        dmx_break::Break,
        geometry::{Geometry, Offsets, Type},
    };

    use super::*;

    #[test]
    fn mode_master() {
        let input = r#"
<FixtureType>
    <DMXModes>
        <DMXMode Description="not a Name." Geometry="Body" Name="Mode 1">
            <DMXChannels>
                <DMXChannel DMXBreak="1" Geometry="Beam" Highlight="127/1" InitialFunction="Beam_Dimmer.Dimmer.Strobe" Offset="1">
                    <LogicalChannel Attribute="Dimmer" DMXChangeTimeLimit="0.000000" Master="Grand" MibFade="0.000000" Snap="No">
                        <ChannelFunction Attribute="Dimmer" CustomName="" DMXFrom="0/1" Default="0/1" Max="1.000000" Min="0.000000" Name="Dimmer" OriginalAttribute="" PhysicalFrom="0.000000" PhysicalTo="1.000000" RealAcceleration="0.000000" RealFade="0.000000">
                            <ChannelSet DMXFrom="0/1" Name="closed" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="1/1" Name="" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="127/1" Name="open" WheelSlotIndex="0"/>
                        </ChannelFunction>
                        <ChannelFunction Attribute="StrobeModeShutter" CustomName="" DMXFrom="128/1" Default="51200/2" Max="1.000000" Min="1.000000" Name="Strobe" OriginalAttribute="" PhysicalFrom="1.000000" PhysicalTo="1.000000" RealAcceleration="0.000000" RealFade="0.000000">
                        </ChannelFunction>
                    </LogicalChannel>
                </DMXChannel>
                <DMXChannel DMXBreak="1" Geometry="Beam" Highlight="0/1" InitialFunction="Beam_StrobeFrequency.StrobeFrequency.StrobeFrequency" Offset="2,3">
                    <LogicalChannel Attribute="StrobeFrequency" DMXChangeTimeLimit="0.000000" Master="Grand" MibFade="0.000000" Snap="No">
                        <ChannelFunction Attribute="StrobeFrequency" CustomName="" DMXFrom="0/1" Default="0/1" Max="1.000000" Min="0.000000" 
                                Name="StrobeFrequency" OriginalAttribute="" PhysicalFrom="0.000000" PhysicalTo="1.000000" RealAcceleration="0.000000" RealFade="0.000000"
                                ModeMaster="Beam_Dimmer" ModeFrom="128/1" ModeTo="65535/2">
                            <ChannelSet DMXFrom="0/1" Name="slowest" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="1/1" Name="" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="127/1" Name="fastest" WheelSlotIndex="0"/>
                        </ChannelFunction>
                        <ChannelFunction Attribute="NoFeature" CustomName="" DMXFrom="0/1" Default="0/2" Max="0.000000" Min="0.000000" 
                                Name="NoFeature Name" OriginalAttribute="" PhysicalFrom="0.000000" PhysicalTo="0.000000" RealAcceleration="0.000000" RealFade="0.000000" 
                                ModeMaster="Beam_Dimmer.Dimmer.Dimmer" ModeFrom="0/1" ModeTo="127/1"
                        />
                    </LogicalChannel>
                </DMXChannel>
            </DMXChannels>
        </DMXMode>
    </DMXModes>
</FixtureType>"#;
        let doc = roxmltree::Document::parse(input).unwrap();
        let ft = doc.root_element();
        let mut parsed = ParsedGdtf::default();

        let body_index = parsed
            .gdtf
            .geometries
            .add_top_level(Geometry {
                name: "Body".into_valid(),
                t: Type::General,
            })
            .unwrap();
        let beam_index = parsed
            .gdtf
            .geometries
            .add(
                Geometry {
                    name: "Beam".into_valid(),
                    t: Type::General,
                },
                body_index,
            )
            .unwrap();
        parsed.parse_dmx_modes(ft);

        assert_eq!(parsed.problems.len(), 0);

        let mut modes = parsed.gdtf.dmx_modes().iter();
        let mode = modes.next().expect("at least one mode present");
        assert!(
            matches!(modes.next(), None),
            "not more than one mode present"
        );

        assert_eq!(mode.name, "Mode 1");
        assert_eq!(mode.description, "not a Name.");
        assert_eq!(mode.geometry(), &body_index);

        assert_eq!(mode.subfixtures.len(), 0);

        let mut channels = mode.channels.iter();
        let dimmer = channels.next().expect("first channel");
        assert_eq!(dimmer.name, "Beam_Dimmer");
        assert_eq!(dimmer.offsets.first().expect("one offset"), &0);
        assert_eq!(dimmer.bytes, 1);
        assert_eq!(dimmer.bytes as usize, dimmer.offsets.len());
        assert_eq!(dimmer.default, 200); // 200/1 = 51200/2
        assert_eq!(dimmer.channel_functions.len(), 3); // 1 raw + 2 normal

        let dimmer_chf = mode
            .channel_functions
            .node_weight(*dimmer.channel_functions.get(1).unwrap())
            .unwrap();
        assert_eq!(dimmer_chf.name, "Dimmer");
        assert_eq!(dimmer_chf.dmx_from, 0);
        assert_eq!(dimmer_chf.dmx_to, 127);
        assert_eq!(dimmer_chf.geometry, beam_index);
        let strobe_chf = mode
            .channel_functions
            .node_weight(*dimmer.channel_functions.get(2).unwrap())
            .unwrap();
        assert_eq!(strobe_chf.name, "Strobe");
        assert_eq!(strobe_chf.dmx_from, 128);
        assert_eq!(strobe_chf.dmx_to, 255);
        assert_eq!(strobe_chf.geometry, beam_index);

        let freq = channels.next().expect("second channel");
        let freq_chf = mode
            .channel_functions
            .node_weight(*freq.channel_functions.get(1).unwrap())
            .unwrap();
        assert_eq!(freq_chf.name, "StrobeFrequency");
        assert_eq!(freq_chf.dmx_from, 0);
        assert_eq!(freq_chf.dmx_to, 65535);
        let nof_chf = mode
            .channel_functions
            .node_weight(*freq.channel_functions.get(2).unwrap())
            .unwrap();
        assert_eq!(nof_chf.name, "NoFeature Name");
        assert_eq!(nof_chf.dmx_from, 0);
        assert_eq!(nof_chf.dmx_to, 65535);

        let raw_dimmer_ind = *dimmer.channel_functions.get(0).unwrap();
        let strobe_freq_ind = *freq.channel_functions.get(1).unwrap();
        let edge_ind = mode
            .channel_functions
            .find_edge(raw_dimmer_ind, strobe_freq_ind)
            .unwrap();
        let edge = mode.channel_functions.edge_weight(edge_ind).unwrap();
        assert_eq!(edge.from, 128);
        assert_eq!(edge.to, 255);

        let dimmer_ind = *dimmer.channel_functions.get(1).unwrap();
        let nof_ind = *freq.channel_functions.get(2).unwrap();
        let edge_ind = mode
            .channel_functions
            .find_edge(dimmer_ind, nof_ind)
            .unwrap();
        let edge = mode.channel_functions.edge_weight(edge_ind).unwrap();
        assert_eq!(edge.from, 0);
        assert_eq!(edge.to, 127);

        assert!(matches!(channels.next(), None), "no more channels");
    }

    #[test]
    fn subfixtures() {
        let input = r#"
<FixtureType>
    <DMXModes>
        <DMXMode Description="not a Name." Geometry="Body" Name="Mode 1">
            <DMXChannels>
                <DMXChannel DMXBreak="1" Geometry="AbstractGeometry" Highlight="255/1" InitialFunction="AbstractGeometry_Dimmer.Dimmer.Dimmer" Offset="1">
                    <LogicalChannel Attribute="Dimmer" DMXChangeTimeLimit="0.000000" Master="Grand" MibFade="0.000000" Snap="No">
                        <ChannelFunction Attribute="Dimmer" CustomName="" DMXFrom="0/1" Default="0/1" Max="1.000000" Min="0.000000" Name="Dimmer" OriginalAttribute="" PhysicalFrom="0.000000" PhysicalTo="1.000000" RealAcceleration="0.000000" RealFade="0.000000">
                            <ChannelSet DMXFrom="0/1" Name="closed" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="1/1" Name="" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="255/1" Name="open" WheelSlotIndex="0"/>
                        </ChannelFunction>
                    </LogicalChannel>
                </DMXChannel>
            </DMXChannels>
        </DMXMode>
    </DMXModes>
</FixtureType>"#;
        let doc = roxmltree::Document::parse(input).unwrap();
        let ft = doc.root_element();
        let mut parsed = ParsedGdtf::default();
        let body_index = parsed
            .gdtf
            .geometries
            .add_top_level(Geometry {
                name: "Body".into_valid(),
                t: Type::General,
            })
            .unwrap();
        let abstract_index = parsed
            .gdtf
            .geometries
            .add_top_level(Geometry {
                name: "AbstractGeometry".into_valid(),
                t: Type::General,
            })
            .unwrap();
        let ref1_index = parsed
            .gdtf
            .geometries
            .add(
                Geometry {
                    name: "Pixel1".into_valid(),
                    t: Type::Reference {
                        offsets: Offsets {
                            normal: HashMap::from([(Break::try_from(1).unwrap(), 1)]),
                            overwrite: None,
                        },
                    },
                },
                body_index,
            )
            .unwrap();
        let ref2_index = parsed
            .gdtf
            .geometries
            .add(
                Geometry {
                    name: "Pixel2".into_valid(),
                    t: Type::Reference {
                        offsets: Offsets {
                            normal: HashMap::from([(Break::try_from(1).unwrap(), 2)]),
                            overwrite: None,
                        },
                    },
                },
                body_index,
            )
            .unwrap();
        parsed
            .gdtf
            .geometries
            .add_template_relationship(abstract_index, ref1_index)
            .unwrap();
        parsed
            .gdtf
            .geometries
            .add_template_relationship(abstract_index, ref2_index)
            .unwrap();

        parsed.parse_dmx_modes(ft);

        assert!(parsed.problems.is_empty());

        let modes = parsed.gdtf.dmx_modes();

        let mode = modes.first().expect("at least one mode present");

        assert_eq!(mode.channels.len(), 0);
        assert_eq!(mode.subfixtures.len(), 2);
        assert_eq!(mode.channel_functions.node_count(), 4);

        assert_eq!(mode.subfixtures.get(0).unwrap().name, "Pixel1");
        assert_eq!(mode.subfixtures.get(0).unwrap().channels.len(), 1);
        assert_eq!(
            mode.subfixtures
                .get(0)
                .unwrap()
                .channels
                .first()
                .unwrap()
                .name,
            "Pixel1_Dimmer"
        );
        assert_eq!(mode.subfixtures.get(1).unwrap().name, "Pixel2");
        assert_eq!(
            mode.subfixtures
                .get(1)
                .unwrap()
                .channels
                .first()
                .unwrap()
                .name,
            "Pixel2_Dimmer"
        );

        let chf_names: Vec<Name> = mode
            .channel_functions
            .node_weights()
            .map(|chf| chf.name.clone())
            .collect();

        dbg!(&chf_names);
        assert!(chf_names.contains(&"Pixel1_Dimmer".into_valid()));
        assert!(chf_names.contains(&"Pixel2_Dimmer".into_valid()));
        assert_eq!(chf_names.iter().filter(|s| s == &"Dimmer").count(), 2);

        // TODO test that LogicalChannel attribute/name is unique in Channel

        // TODO test that ChannelFunction name is unique in LogicalChannel (check before resolving modemaster)

        // TODO what happens if the modeMaster-referenced Channel or ChannelFunction is a template?
        // Then it can only work out if they are in the same subfixture and they reference in the instantiated form with 1:1 mapping

        // TODO what happens if a Channel references a Geometry that is a child of a template top-level geometry, do we pick
        // that up and also treat it as a template channel? -> We should probably instantiate GeometryReference nodes as
        // the Geometry (including its subtree) they reference

        // TODO test geometry renaming and lookup with DMXChannels
    }

    #[test]
    fn default_channel_function_name() {
        let input = r#"
<FixtureType>
    <DMXModes>
        <DMXMode Description="not a Name." Geometry="Body" Name="Mode 1">
            <DMXChannels>
                <DMXChannel DMXBreak="1" Geometry="Body" Highlight="127/1" InitialFunction="Body_Dimmer.Dimmer.Dimmer 1" Offset="1">
                    <LogicalChannel Attribute="Dimmer" DMXChangeTimeLimit="0.000000" Master="Grand" MibFade="0.000000" Snap="No">
                        <ChannelFunction Attribute="Dimmer" CustomName="" DMXFrom="0/1" Default="0/1" Max="1.000000" Min="0.000000" OriginalAttribute="" PhysicalFrom="0.000000" PhysicalTo="1.000000" RealAcceleration="0.000000" RealFade="0.000000">
                            <ChannelSet DMXFrom="0/1" Name="closed" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="1/1" Name="" WheelSlotIndex="0"/>
                            <ChannelSet DMXFrom="127/1" Name="open" WheelSlotIndex="0"/>
                        </ChannelFunction>
                        <ChannelFunction Attribute="StrobeModeShutter" CustomName="" DMXFrom="128/1" Default="51200/2" Max="1.000000" Min="1.000000" Name="Strobe" OriginalAttribute="" PhysicalFrom="1.000000" PhysicalTo="1.000000" RealAcceleration="0.000000" RealFade="0.000000">
                        </ChannelFunction>
                    </LogicalChannel>
                </DMXChannel>
            </DMXChannels>
        </DMXMode>
    </DMXModes>
</FixtureType>"#;
        let doc = roxmltree::Document::parse(input).unwrap();
        let ft = doc.root_element();
        let mut parsed = ParsedGdtf::default();
        let _body_index = parsed
            .gdtf
            .geometries
            .add_top_level(Geometry {
                name: "Body".into_valid(),
                t: Type::General,
            })
            .unwrap();
        parsed.parse_dmx_modes(ft);

        assert_eq!(parsed.problems.len(), 0);

        let chf_names: Vec<Name> = parsed
            .gdtf
            .dmx_modes()
            .first()
            .unwrap()
            .channel_functions
            .node_weights()
            .map(|chf| chf.name.clone())
            .collect();
        assert!(chf_names.contains(&"Body_Dimmer".into_valid()));
        assert!(chf_names.contains(&"Dimmer 1".into_valid()));
    }
}
