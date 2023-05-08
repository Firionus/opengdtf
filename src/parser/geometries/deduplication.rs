use roxmltree::Node;

use crate::{
    types::name::{IntoValidName, Name},
    Problem,
};
use petgraph::graph::NodeIndex;

use super::{ContinueParsing, GeometriesParser};

#[derive(derive_more::Constructor)]
pub(super) struct Duplicate<'a> {
    /// already parsed 'Name' attribute on xml_node, can't parse again due to side effects on get_name
    name: Name,
    n: Node<'a, 'a>,
    /// None if duplicate is top-level
    parent_graph_ind: Option<NodeIndex>,
    top_level_graph_ind: Option<NodeIndex>,
    duplicate_graph_ind: NodeIndex,
}

impl<'a> GeometriesParser<'a> {
    pub(super) fn parse_duplicates(&mut self) {
        while let Some(dup) = self.duplicates.pop_front() {
            let name_to_increment = match self.try_renaming_with_top_level_name(&dup) {
                Ok(()) => continue,
                Err(name_to_increment) => name_to_increment,
            };

            self.try_renaming_by_incrementing_counter(dup, name_to_increment);
        }
    }

    fn try_renaming_with_top_level_name(&mut self, dup: &Duplicate<'a>) -> Result<(), Name> {
        let top_level_name = dup
            .top_level_graph_ind
            .filter(|top_level| !self.renamed_top_level_geometries.contains(top_level))
            .filter(|duplicate_top_level| {
                let original_top_level = self
                    .geometries
                    .top_level_geometry_index(dup.duplicate_graph_ind);
                original_top_level != *duplicate_top_level
            })
            .and_then(|duplicate_top_level| {
                Some(
                    self.geometries
                        .graph()
                        .node_weight(duplicate_top_level)?
                        .name
                        .clone(),
                )
            })
            .ok_or_else(|| dup.name.clone())?;

        let suggested_name = format!("{} (in {top_level_name})", &dup.name).into_valid();

        if self.geometries.names().contains_key(&suggested_name) {
            return Err(suggested_name);
        }

        self.handle_renamed_geometry(dup, &suggested_name);
        self.rename_lookup
            .insert((top_level_name, dup.name.clone()), suggested_name.clone());
        Ok(())
    }

    fn try_renaming_by_incrementing_counter(
        &mut self,
        dup: Duplicate<'a>,
        name_to_increment: Name,
    ) {
        for dedup_ind in 1..10_000 {
            let incremented_name =
                format!("{name_to_increment} (duplicate {dedup_ind})").into_valid();
            if !self.geometries.names().contains_key(&incremented_name) {
                self.handle_renamed_geometry(&dup, &incremented_name);
                return;
            }
        }
        Problem::DuplicateGeometryName(dup.name.clone())
            .at(&dup.n)
            .handled_by("deduplication failed, ignoring node", self.problems);
    }

    fn handle_renamed_geometry(&mut self, dup: &Duplicate<'a>, suggested_name: &Name) {
        let problem = Problem::DuplicateGeometryName(dup.name.clone()).at(&dup.n);

        if let Some((graph_ind, continue_parsing)) =
            self.add_named_geometry(dup.n, suggested_name.clone(), dup.parent_graph_ind)
        {
            if self.geometries.is_top_level(graph_ind) {
                self.renamed_top_level_geometries.insert(graph_ind);
            }

            problem.handled_by(format!("renamed to '{suggested_name}'"), self.problems);

            if let ContinueParsing::Children = continue_parsing {
                self.add_children(
                    dup.n,
                    graph_ind,
                    dup.top_level_graph_ind.unwrap_or(graph_ind),
                );
            }
        } else {
            problem.handled_by(
                format!(
                    "renamed to '{suggested_name}' but still ignoring node due to some other error"
                ),
                self.problems,
            )
        }
    }
}
