use std::{collections::HashMap, num::NonZeroU8};

use opengdtf::{DmxAddress, Gdtf, Geometry, GeometryType};

#[test]
fn add_geometries() -> anyhow::Result<()> {
    let mut gdtf = Gdtf::default();

    let geometry = Geometry {
        name: "test".try_into()?,
        t: GeometryType::Geometry {
            children: Vec::new(),
        },
    };
    gdtf.add_top_level_geometry(geometry.clone())?;
    assert_eq!(gdtf.geometry(&geometry.name), Some(&geometry));

    let template = Geometry {
        name: "template".try_into()?,
        t: GeometryType::Geometry {
            children: Vec::new(),
        },
    };
    gdtf.add_top_level_geometry(template.clone())?;
    assert_eq!(gdtf.geometry(&template.name), Some(&template));

    let child = Geometry {
        name: "child".try_into()?,
        t: GeometryType::Geometry {
            children: Vec::new(),
        },
    };
    gdtf.add_child_geometry(&geometry.name, child.clone())?;
    assert_eq!(gdtf.geometry(&child.name), Some(&child));
    assert!(
        matches!(&gdtf.geometry(&geometry.name).unwrap().t, GeometryType::Geometry { children } if children[0] == child)
    );

    let offset = DmxAddress::try_from(1).unwrap();
    let default_break = NonZeroU8::new(1).unwrap();
    let overwrite = (default_break, offset.clone());
    let mut offsets = HashMap::new();
    offsets.insert(default_break, offset);
    let reference = Geometry {
        name: "reference".try_into()?,
        t: GeometryType::GeometryReference {
            geometry: template.name.clone(),
            overwrite,
            offsets,
        },
    };
    gdtf.add_child_geometry(&geometry.name, reference.clone())?;
    assert_eq!(gdtf.geometry(&reference.name), Some(&reference));
    assert!(
        matches!(&gdtf.geometry(&geometry.name).unwrap().t, GeometryType::Geometry { children } if children[1] == reference)
    );

    assert!(
        matches!(&gdtf.geometry(&geometry.name).unwrap().t, GeometryType::Geometry { children } if children.len() == 2)
    );
    Ok(())
}
