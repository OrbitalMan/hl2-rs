//! Backend-independent sound requests emitted by Source simulation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundActor {
    pub name: String,
    pub model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundRequest {
    pub name: String,
    pub actor: Option<SoundActor>,
}

impl From<String> for SoundRequest {
    fn from(name: String) -> Self {
        Self { name, actor: None }
    }
}

impl From<&str> for SoundRequest {
    fn from(name: &str) -> Self {
        name.to_owned().into()
    }
}

impl AsRef<str> for SoundRequest {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

impl PartialEq<str> for SoundRequest {
    fn eq(&self, other: &str) -> bool {
        self.name == other
    }
}

impl PartialEq<&str> for SoundRequest {
    fn eq(&self, other: &&str) -> bool {
        self.name == *other
    }
}
