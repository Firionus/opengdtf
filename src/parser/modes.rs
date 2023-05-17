use std::cmp::{max, min};

use itertools::Itertools;
use petgraph::graph::NodeIndex;
use roxmltree::Node;
use thiserror::Error;

use crate::{
    dmx_break::Break,
    dmx_modes::{
        chfs, Channel, ChannelFunction, ChannelFunctions, DmxMode, ModeMaster, Subfixture,
    },
    geometries::Geometries,
    geometry::{Geometry, Type},
    name::{IntoValidName, Name},
    Problem, ProblemAt, Problems,
};

use super::{
    dmx_value::{bytes_max_value, parse_dmx},
    parse_xml::{get_xml_attribute::parse_attribute_content, GetXmlAttribute, GetXmlNode},
    problems::{HandleOption, HandleProblem, TransformUnexpected},
};

pub(crate) struct DmxModesParser<'a> {
    geometries: &'a Geometries,
    modes: &'a mut Vec<DmxMode>,
    problems: &'a mut Problems,
}

impl<'a> DmxModesParser<'a> {
    pub(crate) fn new(
        geometries: &'a mut Geometries,
        modes: &'a mut Vec<DmxMode>,
        problems: &'a mut Problems,
    ) -> Self {
        DmxModesParser {
            geometries,
            modes,
            problems,
        }
    }

    pub(crate) fn parse_from(mut self, fixture_type: &'a Node<'a, 'a>) {
        let modes = match fixture_type.find_required_child("DMXModes") {
            Ok(v) => v,
            Err(p) => {
                p.handled_by("leaving DMX modes empty", self.problems);
                return;
            }
        };

        for (i, mode) in modes
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "DMXMode")
            .enumerate()
        {
            if let Err(p) = self.handle_dmx_mode(mode, i) {
                p.handled_by("ignoring DMX Mode", self.problems);
            };
        }
    }

    fn handle_dmx_mode(&mut self, mode: Node, i: usize) -> Result<(), ProblemAt> {
        let mode_name = mode.name(i, self.problems);
        let description = mode.attribute("Description").unwrap_or("").to_owned();
        let mode_geometry = mode
            .parse_required_attribute::<Name>("Geometry")
            .and_then(|g| {
                self.geometries
                    .get_index(&g)
                    .ok_or(Problem::UnknownGeometry(g).at(&mode))
            })
            .and_then(|i| {
                self.geometries.is_top_level(i).then_some(i).ok_or(
                    Problem::NonTopLevelDmxModeGeometry {
                        geometry: self
                            .geometries
                            .get_by_index(i)
                            .map(|g| g.name.to_owned())
                            .unwrap_or_default(),
                        mode: mode_name.to_owned(),
                    }
                    .at(&mode),
                )
            })?;
        let mut channels = vec![];
        let mut subfixtures = vec![];
        let mut channel_functions: ChannelFunctions = Default::default();
        let mut mode_master_queue = Vec::<(Node, &str, &str, &str, NodeIndex)>::new();
        match mode.find_required_child("DMXChannels") {
            Ok(dmx_channels) => {
                for (_j, channel) in dmx_channels
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "DMXChannel")
                    .enumerate()
                {
                    self.handle_dmx_channel(
                        channel,
                        mode_geometry,
                        &mut channel_functions,
                        &mut mode_master_queue,
                        &mode_name,
                        &mut channels,
                        &mut subfixtures,
                    )?;
                }

                for (chf, mode_master, mode_from, mode_to, chf_index) in mode_master_queue {
                    if let Err(e) = handle_mode_master(
                        chf,
                        mode_master,
                        mode_from,
                        mode_to,
                        chf_index,
                        &channels,
                        &mode_name,
                        &mut channel_functions,
                        self.problems,
                    ) {
                        e.handled_by("ignoring mode master", self.problems);
                    }
                }
            }
            Err(p) => p.handled_by("leaving DMX mode empty", self.problems),
        };
        self.modes.push(DmxMode {
            name: mode_name,
            description,
            geometry: mode_geometry,
            channels,
            subfixtures,
            channel_functions,
        });
        Ok(())
    }

    fn handle_dmx_channel<'b>(
        &mut self,
        channel: Node<'b, 'b>,
        mode_geometry: NodeIndex,
        channel_functions: &mut ChannelFunctions,
        mode_master_queue: &mut Vec<(Node<'b, 'b>, &'b str, &'b str, &'b str, NodeIndex)>,
        mode_name: &Name,
        channels: &mut Vec<Channel>,
        subfixtures: &mut Vec<Subfixture>,
    ) -> Result<(), ProblemAt> {
        let geometry_string = channel
            .required_attribute("Geometry")
            .ok_or_handled_by("using empty string", self.problems)
            .unwrap_or("");
        let channel_geometry = Name::try_from(geometry_string).unwrap_or_else(|e| {
            let valid = e.fixed.to_owned();
            Problem::InvalidAttribute {
                attr: "Geometry".to_owned(),
                tag: "DMXChannel".to_owned(),
                content: geometry_string.to_owned(),
                source: Box::new(e),
                expected_type: "Name".to_owned(),
            }
            .at(&channel)
            .handled_by("converting to valid", self.problems);
            valid
        });

        // TODO look up geometry in geometry rename lookup instead of geometries!
        let geometry_index = self
            .geometries
            .get_index(&channel_geometry)
            .ok_or_else(|| Problem::UnknownGeometry(channel_geometry.to_owned()).at(&channel))
            .ok_or_handled_by("using mode geometry", self.problems)
            .unwrap_or(mode_geometry);

        // GDTF 1.2 says this field should be a "Node" (we call it NamePath)
        // But Attributes aren't nested, so there should only ever be one Name here, with no dot
        let first_logic_attribute: Name = channel
            .find_required_child("LogicalChannel")
            .and_then(|n| n.parse_required_attribute("Attribute"))
            .ok_or_handled_by("using empty", self.problems)
            .unwrap_or_default();

        let dmx_break = channel.attribute("DMXBreak").unwrap_or("1");
        let dmx_break = if dmx_break == "Overwrite" {
            Ok(ChannelBreak::Overwrite)
        } else {
            parse_attribute_content(&channel, dmx_break, "DMXBreak").map(ChannelBreak::Break)
        }
        .ok_or_handled_by("using default", self.problems)
        .unwrap_or_default();
        let offset_string = channel
            .required_attribute("Offset")
            .ok_or_handled_by("using none", self.problems)
            .unwrap_or("None");
        let mut offsets: Vec<u16> = match offset_string {
            "None" | "" => vec![],
            s => s
                .split(',')
                .filter_map(|si| {
                    si.parse::<u16>()
                        .map_err(|e| Box::new(e) as _)
                        .and_then(|i| {
                            if (1..=512).contains(&i) {
                                Ok(i - 1)
                            } else {
                                Err(Box::new(OffsetError()) as _)
                            }
                        })
                        .map_err(|e| {
                            Problem::InvalidAttribute {
                                attr: "Offset".to_owned(),
                                tag: "DMXChannel".to_owned(),
                                content: offset_string.to_owned(),
                                source: e,
                                expected_type: "u16".to_owned(),
                            }
                            .at(&channel)
                            .handled_by("omitting value", self.problems)
                        })
                        .ok()
                })
                .collect(),
        };
        let channel_bytes = if offsets.is_empty() {
            4 // use maximum resolution for virtual channel
        } else if offsets.len() > 4 {
            Problem::UnsupportedByteCount(offsets.len())
                .at(&channel)
                .handled_by("using only 4 most significant bytes", self.problems);
            offsets.truncate(4);
            4
        } else {
            offsets.len() as u8
        };
        let max_dmx_value = bytes_max_value(channel_bytes);

        if self.geometries.is_template(geometry_index) {
            for ref_ind in self.geometries.template_references(geometry_index) {
                let reference = self
                    .geometries
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
                    .handled_by("skipping", self.problems);
                    continue;
                };

                let (actual_dmx_break, offsets_offset) = match dmx_break {
                    ChannelBreak::Overwrite => match &reference_offsets.overwrite {
                        Some(o) => (o.dmx_break, o.offset),
                        None => {
                            Problem::MissingBreakInReference {
                                br: "Overwrite".into(),
                                ch: format!("{channel_geometry}_{first_logic_attribute}")
                                    .into_valid(),
                                mode: mode_name.to_owned(),
                            }
                            .at(&channel)
                            .handled_by("skipping", self.problems);
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
                                    ch: format!(
                                        "{channel_geometry}_{}",
                                        first_logic_attribute.to_owned()
                                    )
                                    .into_valid(),
                                    mode: mode_name.to_owned(),
                                }
                                .at(&channel)
                                .handled_by("skipping", self.problems);
                                continue;
                            }
                        },
                    ),
                };
                let dmx_channel = self.abstract_dmx_channel(
                    ref_ind,
                    max_dmx_value,
                    channel_functions,
                    channel,
                    channel_bytes,
                    mode_master_queue,
                    mode_name,
                    offsets
                        .iter()
                        .map(|o| o + (offsets_offset as u16) - 1)
                        .collect(),
                    actual_dmx_break,
                    first_logic_attribute.to_owned(),
                )?;
                let sf: &mut Subfixture =
                    if let Some(sf) = subfixtures.iter_mut().find(|sf| sf.geometry == ref_ind) {
                        sf
                    } else {
                        subfixtures.push(Subfixture {
                            name: reference.name.to_owned(),
                            channels: vec![],
                            geometry: ref_ind,
                        });
                        subfixtures
                            .iter_mut()
                            .last()
                            .ok_or_unexpected_at("just pushed", &channel)?
                    };

                sf.channels.push(dmx_channel);
            }
        } else {
            let actual_dmx_break = match dmx_break {
                ChannelBreak::Break(b) => b,
                ChannelBreak::Overwrite => Err(Problem::InvalidBreakOverwrite {
                    ch: format!("{channel_geometry}_{first_logic_attribute}").into_valid(),
                    mode: mode_name.to_owned(),
                }
                .at(&channel))?,
            };

            let channel = self.abstract_dmx_channel(
                geometry_index,
                max_dmx_value,
                channel_functions,
                channel,
                channel_bytes,
                mode_master_queue,
                mode_name,
                offsets,
                actual_dmx_break,
                first_logic_attribute,
            )?;
            channels.push(channel);
        }

        Ok(())
    }

    fn abstract_dmx_channel<'b>(
        &mut self,
        geometry_index: NodeIndex,
        max_dmx_value: u32,
        channel_functions: &mut crate::checked_graph::CheckedGraph<ChannelFunction, ModeMaster>,
        channel: Node<'b, 'b>,
        channel_bytes: u8,
        mode_master_queue: &mut Vec<(Node<'b, 'b>, &'b str, &'b str, &'b str, NodeIndex)>,
        mode_name: &Name,
        offsets: Vec<u16>,
        dmx_break: Break,
        first_logic_attribute: Name,
    ) -> Result<Channel, ProblemAt> {
        let geometry = self
            .geometries
            .get_by_index(geometry_index)
            .unexpected_err_at(&channel)?;

        let name = format!("{}_{first_logic_attribute}", geometry.name).into_valid();
        let mut channel_function_ids: Vec<NodeIndex> = Default::default();
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
        let raw_idx = channel_functions
            .add_node(raw_channel_function)
            .unexpected_err_at(&channel)?;
        channel_function_ids.push(raw_idx);
        for (_k, logical_channel) in channel
            .children()
            .filter(|n| n.has_tag_name("LogicalChannel"))
            .enumerate()
        {
            // TODO read Snap, Master, MibFade, DMXChangeTimeLimit
            let mut chf_iter = logical_channel
                .children()
                .filter(|n| n.has_tag_name("ChannelFunction"))
                .enumerate()
                .peekable();

            while let Some((l, chf)) = chf_iter.next() {
                let chf_attr = chf.attribute("Attribute").unwrap_or("NoFeature");
                let original_attribute = chf.attribute("OriginalAttribute").unwrap_or("");
                let chf_name: Name = chf
                    .parse_attribute("Name")
                    .and_then(|r| r.ok_or_handled_by("using default", self.problems))
                    .unwrap_or_else(|| format!("{chf_attr} {}", l + 1).into_valid());

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
                            .ok_or_handled_by("using default 0", self.problems)
                    })
                    .unwrap_or(0);
                // The convention to use the next ChannelFunction in XML order for DMXTo is not official
                // but probably correct for GDTF Builder files.
                // see https://github.com/mvrdevelopment/spec/issues/103#issuecomment-985361192
                let dmx_to = chf_iter.peek().and_then(|(_, next_chf)| {
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
                        .ok_or_handled_by("using maximum channel value for DMXTo of previous channel function", self.problems)
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
                            .ok_or_handled_by("using default 0", self.problems)
                    })
                    .unwrap_or(0);

                let phys_from = chf
                    .parse_attribute("PhysicalFrom")
                    .transpose()
                    .ok_or_handled_by("using default 0", self.problems)
                    .flatten()
                    .unwrap_or(0.);
                let phys_to = chf
                    .parse_attribute("PhysicalTo")
                    .transpose()
                    .ok_or_handled_by("using default 1", self.problems)
                    .flatten()
                    .unwrap_or(1.);

                let chf_index = channel_functions
                    .add_node(ChannelFunction {
                        name: chf_name.to_owned(),
                        geometry: geometry_index,
                        attr: chf_attr.to_owned(),
                        original_attr: original_attribute.to_owned(),
                        dmx_from,
                        dmx_to,
                        phys_from,
                        phys_to,
                        default,
                    })
                    .unexpected_err_at(&chf)?;

                channel_function_ids.push(chf_index);

                if let Some(mode_master) = chf.attribute("ModeMaster") {
                    if let (Some(mode_from), Some(mode_to)) =
                        (chf.attribute("ModeFrom"), chf.attribute("ModeTo"))
                    {
                        mode_master_queue.push((
                            chf.to_owned(),
                            mode_master,
                            mode_from,
                            mode_to,
                            chf_index,
                        ));
                    } else {
                        Problem::MissingModeFromOrTo(chf_name.as_str().to_owned())
                            .at(&chf)
                            .handled_by("ignoring ModeMaster", self.problems)
                    }
                }
            }
        }
        let initial_function = channel.attribute("InitialFunction").and_then(|s| {
            let mut it = s.split('.');
            it.next_tuple()
                .filter(|(ch, _lch, _chf)| (&name == ch))
                .and_then(|(_ch, _lch, chf)| {
                    for v in chfs(&channel_function_ids, &*channel_functions) {
                        match v {
                            Ok(v) => {
                                if v.1.name == chf {
                                    return Some(v.0);
                                }
                            }
                            Err(p) => {
                                p.at(&channel).handled_by("using default", self.problems);
                                return None;
                            }
                        }
                    }
                    None
                })
                .or_else(|| {
                    Problem::InvalidInitialFunction {
                        s: s.into(),
                        channel: name.clone(),
                        mode: mode_name.clone(),
                    }
                    .at(&channel)
                    .handled_by(
                        "using default (first channel function of first logical channel)",
                        self.problems,
                    );
                    None
                })
        });
        let initial_function = match initial_function {
            Some(v) => v,
            None => {
                let mut it = channel_function_ids.iter();
                let raw = it.next();
                if let Some(default) = it.next() {
                    *default
                } else {
                    *raw.ok_or_unexpected_at("no raw channel function", &channel)?
                }
            }
        };
        let default = channel_functions
            .node_weight(initial_function)
            .ok_or_unexpected_at("invalid initial channel function index", &channel)?
            .default;
        Ok(Channel {
            name,
            dmx_break,
            offsets,
            channel_functions: channel_function_ids,
            bytes: channel_bytes,
            initial_function,
            default,
        })
    }
}

fn handle_mode_master(
    chf: Node,
    mode_master: &str,
    mode_from: &str,
    mode_to: &str,
    chf_index: NodeIndex,
    channels: &[Channel],
    mode_name: &Name,
    channel_functions: &mut ChannelFunctions,
    problems: &mut Problems,
) -> Result<(), ProblemAt> {
    let mut master_path = mode_master.split('.');
    let master_channel_name: Name = master_path.next().unwrap_or("Default Channel").into_valid();
    let dependency_channel: &Channel = channels
        .iter()
        .find(|ch| ch.name == master_channel_name)
        .ok_or_else(|| Problem::UnknownChannel(master_channel_name, mode_name.clone()).at(&chf))?;

    let (master, master_index): (&ChannelFunction, NodeIndex) = if master_path.next().is_some() {
        // reference to channel function
        let dependency_chf_name = master_path.next().ok_or_else(|| {
            Problem::InvalidAttribute {
                attr: "ModeMaster".into(),
                tag: "ChannelFunction".into(),
                content: mode_master.into(),
                source: ModeMasterParseError {}.into(),
                expected_type: "Node".into(),
            }
            .at(&chf)
        })?;
        let mut master_chf = Default::default();
        // TODO replace with custom method on Channel
        for ni in dependency_channel.channel_functions.iter() {
            let chf_candidate = channel_functions.node_weight(*ni).ok_or_else(|| {
                Problem::Unexpected("Invalid Channel Function Index".into()).at(&chf)
            })?;
            if chf_candidate.name == dependency_chf_name {
                master_chf = Some((chf_candidate, *ni));
                break;
            }
        }
        master_chf.ok_or_else(|| {
            Problem::UnknownChannelFunction {
                name: dependency_chf_name.into_valid(),
                mode: mode_name.clone(),
            }
            .at(&chf)
        })?
    } else {
        // reference to channel, so in our interpretation to the raw dmx channel function
        dependency_channel
            .channel_functions
            .get(0)
            .and_then(|i| channel_functions.node_weight(*i).map(|chf| (chf, *i)))
            .ok_or_else(|| Problem::Unexpected("no raw dmx channel function".into()).at(&chf))?
    };

    let mode_from = parse_dmx(mode_from, dependency_channel.bytes)
        .map_err(|e| {
            Problem::InvalidAttribute {
                attr: "ModeFrom".into(),
                tag: "ChannelFunction".into(),
                content: mode_from.into(),
                source: Box::new(e),
                expected_type: "DMXValue".into(),
            }
            .at(&chf)
        })
        .ok_or_handled_by("using default 0", problems)
        .unwrap_or(0);
    let mode_to = parse_dmx(mode_to, dependency_channel.bytes)
        .map_err(|e| {
            Problem::InvalidAttribute {
                attr: "ModeTo".into(),
                tag: "ChannelFunction".into(),
                content: mode_to.into(),
                source: Box::new(e),
                expected_type: "DMXValue".into(),
            }
            .at(&chf)
        })
        .ok_or_handled_by("using default 0", problems)
        .unwrap_or(0);

    let clipped_mode_from = max(mode_from, master.dmx_from);
    let clipped_mode_to = min(mode_to, master.dmx_to);

    if clipped_mode_to < clipped_mode_from {
        let chf_name = channel_functions
            .node_weight(chf_index)
            .map(|chf| chf.name.to_owned())
            .ok_or_unexpected_at("invalid chf index for mode master handler", &chf)?;
        return Err(Problem::UnreachableChannelFunction {
            name: chf_name,
            dmx_mode: mode_name.to_owned(),
            mode_from,
            mode_to,
        }
        .at(&chf));
    }

    channel_functions
        .add_edge(
            master_index,
            chf_index,
            ModeMaster {
                from: clipped_mode_from,
                to: clipped_mode_to,
            },
        )
        .unexpected_err_at(&chf)?;
    Ok(())
}

#[derive(Debug, Error)]
#[error("mode master attribute must contain either zero or two period separators")]
pub struct ModeMasterParseError();

#[derive(Debug, Error)]
#[error("DXM address offsets must be between 1 and 512")]
pub struct OffsetError();

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
        let mut problems: Problems = vec![];
        let mut geometries = Geometries::default();
        let body_index = geometries
            .add_top_level(Geometry {
                name: "Body".into_valid(),
                t: Type::General,
            })
            .unwrap();
        let beam_index = geometries
            .add(
                Geometry {
                    name: "Beam".into_valid(),
                    t: Type::General,
                },
                body_index,
            )
            .unwrap();
        let mut modes = Vec::<DmxMode>::new();
        DmxModesParser::new(&mut geometries, &mut modes, &mut problems).parse_from(&ft);

        assert_eq!(problems.len(), 0);

        let mut modes = modes.iter();
        let mode = modes.next().expect("at least one mode present");
        assert!(
            matches!(modes.next(), None),
            "not more than one mode present"
        );

        assert_eq!(mode.name, "Mode 1");
        assert_eq!(mode.description, "not a Name.");
        assert_eq!(mode.geometry, body_index);

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
        let mut problems: Problems = vec![];
        let mut geometries = Geometries::default();
        let body_index = geometries
            .add_top_level(Geometry {
                name: "Body".into_valid(),
                t: Type::General,
            })
            .unwrap();
        let abstract_index = geometries
            .add_top_level(Geometry {
                name: "AbstractGeometry".into_valid(),
                t: Type::General,
            })
            .unwrap();
        let ref1_index = geometries
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
        let ref2_index = geometries
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
        geometries
            .add_template_relationship(abstract_index, ref1_index)
            .unwrap();
        geometries
            .add_template_relationship(abstract_index, ref2_index)
            .unwrap();

        let mut modes = Vec::<DmxMode>::new();
        DmxModesParser::new(&mut geometries, &mut modes, &mut problems).parse_from(&ft);

        let mode = modes.first().expect("at least one mode present");

        assert_eq!(mode.channels.len(), 0);
        assert_eq!(mode.subfixtures.len(), 2);
        assert_eq!(mode.channel_functions.node_count(), 4);

        // TODO test the rest (names, etc.)

        // TODO test that mode master and templating is correctly working together. See Kotlin impl for details.
    }
}
