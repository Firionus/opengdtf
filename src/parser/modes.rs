use roxmltree::Node;

use crate::{
    dmx_modes::{Channel, ChannelBreak, ChannelFunctions, DmxMode, Subfixture},
    geometries::Geometries,
    geometry,
    types::{
        dmx_break::{self, Break},
        name::{IntoValidName, Name},
        name_path::NamePath,
    },
    Problem, ProblemAt, Problems,
};

use super::{
    parse_xml::{get_xml_attribute::parse_attribute_content, GetXmlAttribute, GetXmlNode},
    problems::HandleProblem,
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
            let name = mode.name(i, self.problems);
            let description = mode
                .required_attribute("Description")
                .ok_or_handled_by("using empty string", self.problems)
                .unwrap_or("")
                .to_owned();

            // let geometry = match mode.required_attribute("Geometry") {
            //     Ok(v) => self.geometries.get_index(v),
            //     Err(p) => {
            //         p.handled_by("omitting DMX mode", problems);
            //         continue
            //     }
            // }

            let geometry = mode
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
                            mode: name.to_owned(),
                        }
                        .at(&mode),
                    )
                });
            let geometry = match geometry {
                Ok(i) => i,
                Err(p) => {
                    p.handled_by("omitting DMX mode", self.problems);
                    continue;
                }
            };

            let mut channels = vec![];
            let mut subfixtures = vec![];
            let mut channel_functions: ChannelFunctions = Default::default();

            match mode.find_required_child("DMXChannels") {
                Ok(dmx_channels) => {
                    for (j, channel) in dmx_channels
                        .children()
                        .filter(|n| n.is_element() && n.tag_name().name() == "DMXChannel")
                        .enumerate()
                    {
                        let geometry_string = channel
                            .required_attribute("Geometry")
                            .ok_or_handled_by("using empty string", self.problems)
                            .unwrap_or("");
                        let geometry = Name::try_from(geometry_string).unwrap_or_else(|e| {
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
                        // GDTF 1.2 says this field should be a "Node" (we call it NamePath)
                        // But Attributes aren't nested, so there should only ever be one Name here, with no dot
                        let first_logic_attribute: Name = channel
                            .children()
                            .find(|n| n.is_element() && n.has_tag_name("LogicalChannel"))
                            .ok_or_else(|| {
                                Problem::XmlNodeMissing {
                                    missing: "LogicalChannel".to_owned(),
                                    parent: "DMXChannel".to_owned(),
                                }
                                .at(&channel)
                            })
                            // TODO the pattern of "parse_required_attribute" and handling errors by using the default should be factored out
                            // it should bringt great benefits to productivity
                            .and_then(|n| n.parse_required_attribute("Attribute"))
                            .ok_or_handled_by("using default", self.problems)
                            .unwrap_or_default();
                        // validation that geometry and attributes exist is not really necessary,
                        // since it's just a name and these fields are just metadata anyway
                        let name = format!("{geometry}_{first_logic_attribute}");

                        let dmx_break = channel.attribute("DMXBreak").unwrap_or("1");
                        let dmx_break = if dmx_break == "Overwrite" {
                            Ok(ChannelBreak::Overwrite)
                        } else {
                            parse_attribute_content(&channel, dmx_break, "DMXBreak")
                                .map(ChannelBreak::Break)
                        }
                        .ok_or_handled_by("using default", self.problems)
                        .unwrap_or_default();

                        let offset_string = channel
                            .required_attribute("Offset")
                            .ok_or_handled_by("using none", self.problems)
                            .unwrap_or("None");
                        let offsets: Vec<u16> = match offset_string {
                            "None" => vec![],
                            // TODO should this pattern be factored out?
                            s => s
                                .split(',')
                                .filter_map(|si| {
                                    si.parse()
                                        .map_err(|e| {
                                            Problem::InvalidAttribute {
                                                attr: "Offset".to_owned(),
                                                tag: "DMXChannel".to_owned(),
                                                content: offset_string.to_owned(),
                                                source: Box::new(e),
                                                expected_type: "u16".to_owned(),
                                            }
                                            .at(&channel)
                                            .handled_by("omitting value", self.problems)
                                        })
                                        .ok()
                                })
                                .collect(),
                        };

                        let channel_functions = vec![];
                        channels.push(Channel {
                            name,
                            dmx_break,
                            offsets,
                            channel_functions,
                        });
                    }
                }
                Err(p) => p.handled_by("leaving DMX mode empty", self.problems),
            };

            self.modes.push(DmxMode {
                name,
                description,
                geometry,
                channels,
                subfixtures,
                channel_functions,
            })
        }
    }
}
