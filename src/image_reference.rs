use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageReference {
    pub registry: Option<String>,
    pub name: String,
    pub tag: Option<String>,
    pub digest: Option<String>,
}

impl ImageReference {
    pub fn to_canonical_string(&self) -> String {
        let mut s = String::new();
        if let Some(ref reg) = self.registry {
            s.push_str(reg);
            s.push('/');
        }
        s.push_str(&self.name);

        if let Some(ref digest) = self.digest {
            s.push('@');
            s.push_str(digest);
        }
        s
    }

    pub fn to_tag_string(&self) -> String {
        let mut s = String::new();
        if let Some(ref reg) = self.registry {
            s.push_str(reg);
            s.push('/');
        }
        s.push_str(&self.name);
        if let Some(ref tag) = self.tag {
            s.push(':');
            s.push_str(tag);
        }
        s
    }

    pub fn to_full_string(&self) -> String {
        let mut s = self.to_tag_string();
        if let Some(ref digest) = self.digest {
            s.push('@');
            s.push_str(digest);
        }
        s
    }

    pub fn docker_tag_url(&self) -> String {
        format!("docker://{}", self.to_tag_string())
    }

    pub fn docker_canonical_url(&self) -> String {
        format!("docker://{}", self.to_canonical_string())
    }

    pub fn storage_tag_url(&self) -> String {
        format!("containers-storage:{}", self.to_tag_string())
    }

    pub fn storage_full_url(&self) -> String {
        format!("containers-storage:{}", self.to_full_string())
    }
}

pub fn parse_oci_reference(s: &str) -> Option<ImageReference> {
    if s.is_empty() {
        return None;
    }

    let mut remainder = s;
    let mut digest = None;
    let mut tag = None;

    if let Some(idx) = remainder.rfind('@') {
        digest = Some(remainder[idx + 1..].to_string());
        remainder = &remainder[..idx];
    }

    let last_slash_idx = remainder.rfind('/');
    let name_part = if let Some(idx) = last_slash_idx {
        &remainder[idx + 1..]
    } else {
        remainder
    };

    if let Some(idx) = name_part.find(':') {
        tag = Some(name_part[idx + 1..].to_string());
        let tag_start =
            last_slash_idx.unwrap_or(0) + (if last_slash_idx.is_some() { 1 } else { 0 }) + idx;
        remainder = &remainder[..tag_start];
    }

    let mut parts: Vec<&str> = remainder.split('/').collect();

    let registry = if parts.len() > 1 {
        let first = parts[0];
        if first.contains('.') || first.contains(':') || first == "localhost" {
            Some(parts.remove(0).to_string())
        } else {
            None
        }
    } else {
        None
    };

    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let name = parts.join("/");

    Some(ImageReference {
        registry,
        name,
        tag,
        digest,
    })
}
