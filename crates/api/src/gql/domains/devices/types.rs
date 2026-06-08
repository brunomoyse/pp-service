use async_graphql::{Enum, InputObject};

/// Platform a push token was minted on. GraphQL names are `IOS` / `ANDROID` /
/// `WEB`, matching the player app's `PushPlatform`.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DevicePlatform {
    Ios,
    Android,
    Web,
}

impl DevicePlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            DevicePlatform::Ios => "IOS",
            DevicePlatform::Android => "ANDROID",
            DevicePlatform::Web => "WEB",
        }
    }
}

#[derive(InputObject)]
pub struct RegisterDeviceTokenInput {
    /// The Expo push token (e.g. `ExponentPushToken[...]`).
    pub token: String,
    pub platform: DevicePlatform,
}
