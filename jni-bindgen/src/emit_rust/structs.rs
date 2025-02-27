use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write;
use std::io;

use jreflection::class;

use super::fields::Field;
use super::known_docs_url::KnownDocsUrl;
use super::methods::Method;
use crate::emit_rust::Context;
use crate::identifiers::{FieldMangling, RustIdentifier};

#[derive(Debug, Default)]
pub(crate) struct StructPaths {
    pub mod_: String,
    pub struct_name: String,
}

impl StructPaths {
    pub(crate) fn new(context: &Context, class: class::Id) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            mod_: Struct::mod_for(context, class)?,
            struct_name: Struct::name_for(context, class)?,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Struct {
    pub rust: StructPaths,
    pub java: jreflection::Class,
}

fn rust_id(id: &str) -> Result<&str, Box<dyn Error>> {
    Ok(match RustIdentifier::from_str(id) {
        RustIdentifier::Identifier(id) => id,
        RustIdentifier::KeywordRawSafe(id) => id,
        RustIdentifier::KeywordUnderscorePostfix(id) => id,
        RustIdentifier::NonIdentifier(id) => io_data_err!(
            "Unable to add_struct(): java identifier {:?} has no rust equivalent (yet?)",
            id
        )?,
    })
}

impl Struct {
    pub(crate) fn mod_for(_context: &Context, class: class::Id) -> Result<String, Box<dyn Error>> {
        let mut buf = String::new();
        for component in class.iter() {
            match component {
                class::IdPart::Namespace(id) => {
                    if !buf.is_empty() {
                        buf.push_str("::");
                    }
                    buf.push_str(rust_id(id)?);
                }
                class::IdPart::ContainingClass(_) => {}
                class::IdPart::LeafClass(_) => {}
            }
        }
        Ok(buf)
    }

    pub(crate) fn name_for(context: &Context, class: class::Id) -> Result<String, Box<dyn Error>> {
        let rename_to = context
            .config
            .rename_classes
            .get(class.as_str())
            .map(|name| name.as_str())
            .ok_or(());
        let mut buf = String::new();
        for component in class.iter() {
            match component {
                class::IdPart::Namespace(_) => {}
                class::IdPart::ContainingClass(id) => write!(&mut buf, "{}_", rust_id(id)?)?,
                class::IdPart::LeafClass(id) => write!(&mut buf, "{}", rename_to.or_else(|_| rust_id(id))?)?,
            }
        }
        Ok(buf)
    }

    pub(crate) fn new(context: &mut Context, java: jreflection::Class) -> Result<Self, Box<dyn Error>> {
        let rust = StructPaths::new(context, java.path.as_id())?;

        Ok(Self { rust, java })
    }

    pub(crate) fn write(&self, context: &Context, indent: &str, out: &mut impl io::Write) -> io::Result<()> {
        writeln!(out)?;

        // Ignored access_flags: SUPER, SYNTHETIC, ANNOTATION, ABSTRACT

        let keyword = if self.java.is_interface() {
            "interface"
        } else if self.java.is_enum() {
            "enum"
        } else if self.java.is_static() {
            "static java"
        } else if self.java.is_final() {
            "final class"
        } else {
            "class"
        };

        let visibility = if self.java.is_public() { "public" } else { "private" };

        let attributes = (if self.java.deprecated { "#[deprecated] " } else { "" }).to_string();

        let super_path = if let Some(super_path) = self.java.super_path.as_ref() {
            context.java_to_rust_path(super_path.as_id(), &self.rust.mod_).unwrap()
        } else {
            "()".to_owned() // This might only happen for java.lang.Object
        };

        writeln!(out, "{}__jni_bindgen! {{", indent)?;
        if let Some(url) = KnownDocsUrl::from_class(context, self.java.path.as_id()) {
            writeln!(out, "{}    /// {} {} {}", indent, visibility, keyword, url)?;
        } else {
            writeln!(
                out,
                "{}    /// {} {} {}",
                indent,
                visibility,
                keyword,
                self.java.path.as_str()
            )?;
        }
        write!(
            out,
            "{}    {}{} {} {} ({:?}) extends {}",
            indent,
            attributes,
            visibility,
            keyword,
            &self.rust.struct_name,
            self.java.path.as_str().to_string() + "\0",
            super_path
        )?;
        let mut implements = false;
        for interface in &self.java.interfaces {
            if !context.all_classes.contains(interface.as_str()) {
                continue;
            }
            write!(out, ", ")?;
            if !implements {
                write!(out, "implements ")?;
                implements = true;
            }
            write!(
                out,
                "{}",
                &context.java_to_rust_path(interface.as_id(), &self.rust.mod_).unwrap()
            )?;
        }
        writeln!(out, " {{")?;

        let mut id_repeats = HashMap::new();

        let mut methods: Vec<Method> = self
            .java
            .methods
            .iter()
            .map(|m| Method::new(context, &self.java, m))
            .collect();
        let mut fields: Vec<Field> = self
            .java
            .fields
            .iter()
            .map(|f| Field::new(context, &self.java, f))
            .collect();

        for method in &methods {
            if !method.java.is_public() {
                continue;
            } // Skip private/protected methods
            if let Some(name) = method.rust_name() {
                *id_repeats.entry(name.to_owned()).or_insert(0) += 1;
            }
        }

        for field in &fields {
            if !field.java.is_public() {
                continue;
            } // Skip private/protected fields
            match field.rust_names.as_ref() {
                Ok(FieldMangling::ConstValue(name, _)) => {
                    *id_repeats.entry(name.to_owned()).or_insert(0) += 1;
                }
                Ok(FieldMangling::GetSet(get, set)) => {
                    *id_repeats.entry(get.to_owned()).or_insert(0) += 1;
                    *id_repeats.entry(set.to_owned()).or_insert(0) += 1;
                }
                Err(_) => {}
            }
        }

        for method in &mut methods {
            if let Some(name) = method.rust_name() {
                let repeats = *id_repeats.get(name).unwrap_or(&0);
                let overloaded = repeats > 1;
                if overloaded {
                    method.set_mangling_style(context.config.codegen.method_naming_style_collision);
                }
            }

            method.emit(context, indent, &self.rust.mod_, out)?;
        }

        for field in &mut fields {
            field.emit(context, indent, &self.rust.mod_, out)?;
        }

        writeln!(out, "{}    }}", indent)?;
        writeln!(out, "{}}}", indent)?;
        Ok(())
    }
}
