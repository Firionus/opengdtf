use std::cmp::{max, min};

use itertools::Itertools;
use petgraph::graph::NodeIndex;
use roxmltree::Node;
use thiserror::Error;

use crate::{
    dmx_modes::{
        chfs, Channel, ChannelBreak, ChannelFunction, ChannelFunctions, DmxMode, ModeMaster,
    },
    geometries::Geometries,
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
        let first_logic_attribute: Name = channel
            .find_required_child("LogicalChannel")
            .and_then(|n| n.parse_required_attribute("Attribute"))
            .ok_or_handled_by("using empty", self.problems)
            .unwrap_or_default();
        let name = format!("{channel_geometry}_{first_logic_attribute}").into_valid();
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
        let geometry_index = self
            .geometries
            .get_index(&channel_geometry)
            .ok_or_else(|| Problem::UnknownGeometry(channel_geometry.to_owned()).at(&channel))
            .ok_or_handled_by("using mode geometry", self.problems)
            .unwrap_or(mode_geometry);
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
                .filter(|(ch, lch, chf)| (&name == ch))
                .and_then(|(ch, lch, chf)| {
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
        channels.push(Channel {
            name,
            dmx_break,
            offsets,
            channel_functions: channel_function_ids,
            bytes: channel_bytes,
            initial_function,
            default,
        });
        Ok(())
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

#[cfg(test)]
mod tests {
    use crate::geometry::{Geometry, Type};

    use super::*;

    #[test]
    fn basic_mode() {
        let input = r#"
<FixtureType>
    <DMXModes>
        <DMXMode Description="not a Name." Geometry="Body" Name="Mode 1">
            <DMXChannels>
                <DMXChannel DMXBreak="1" Geometry="Beam" Highlight="255/1" InitialFunction="Beam_Dimmer.Dimmer.Dimmer" Offset="1">
                    <LogicalChannel Attribute="Dimmer" DMXChangeTimeLimit="0.000000" Master="Grand" MibFade="0.000000" Snap="No">
                        <ChannelFunction Attribute="Dimmer" CustomName="" DMXFrom="0/1" Default="123/1" Max="1.000000" Min="0.000000" Name="Dimmer" OriginalAttribute="" PhysicalFrom="0.000000" PhysicalTo="1.000000" RealAcceleration="0.000000" RealFade="0.000000">
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
        let beam_index = geometries.add(
            Geometry {
                name: "Beam".into_valid(),
                t: Type::General,
            },
            body_index,
        );
        let mut modes = Vec::<DmxMode>::new();
        //let rename_lookup
        DmxModesParser::new(&mut geometries, &mut modes, &mut problems).parse_from(&ft);

        dbg!(problems);

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
        let dimmer = channels.next().expect("at least one channel");
        assert_eq!(dimmer.name, "Beam_Dimmer");
        assert_eq!(dimmer.offsets.first().expect("one offset"), &0);
        assert_eq!(dimmer.bytes, 1);
        assert_eq!(dimmer.bytes as usize, dimmer.offsets.len());
        assert_eq!(dimmer.default, 123);
    }
}
