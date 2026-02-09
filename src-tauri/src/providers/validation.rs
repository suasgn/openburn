use super::contract::ProviderContract;

pub fn validate_auth_strategy_for_provider(
    provider: &ProviderContract,
    auth_strategy_id: Option<&str>,
) -> Result<(), String> {
    let Some(auth_strategy_id) = auth_strategy_id else {
        return Ok(());
    };

    if provider.supports_auth_strategy(auth_strategy_id) {
        Ok(())
    } else {
        Err(format!(
            "authStrategyId '{}' is not supported by providerId '{}'",
            auth_strategy_id, provider.id
        ))
    }
}

pub fn validate_provider_settings(
    provider: &ProviderContract,
    settings: &serde_json::Value,
) -> Result<(), String> {
    let object = settings
        .as_object()
        .ok_or_else(|| "settings must be a JSON object".to_string())?;

    for required_key in provider.settings.required_keys {
        if !object.contains_key(*required_key) {
            return Err(format!(
                "settings.{} is required for providerId '{}'",
                required_key, provider.id
            ));
        }
    }

    if !provider.settings.allow_additional_keys {
        for key in object.keys() {
            if !provider
                .settings
                .required_keys
                .iter()
                .any(|required_key| required_key == &key.as_str())
            {
                return Err(format!(
                    "settings.{} is not allowed for providerId '{}'",
                    key, provider.id
                ));
            }
        }
    }

    Ok(())
}
