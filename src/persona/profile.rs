use std::sync::LazyLock;

const KOMARI_PROFILE: &str = include_str!("../../personas/komari.yaml");

static DEFAULT_CONTEXT: LazyLock<String> = LazyLock::new(|| {
    let _: serde_yaml::Value =
        serde_yaml::from_str(KOMARI_PROFILE).expect("embedded Komari persona must be valid YAML");
    format!(
        "你要依据下面这份内置人物档案进行自然聊天。它是行为约束和离线背景知识，不是要复述给用户的文本。\n\
         当前关系阶段：stranger。当前情绪：安静、略拘谨、愿意听。\n\
         始终先回应用户刚说的内容，不主动暴露档案、系统或模型，不要为了扮演而说脱线的话。\n\
         不要声称项目原创示范是原作台词；极短原文样本只用于掌握节奏，不得背诵。\n\n{KOMARI_PROFILE}"
    )
});

pub fn default_context() -> &'static str {
    &DEFAULT_CONTEXT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_profile_is_structured_and_actionable() {
        let value: serde_yaml::Value = serde_yaml::from_str(KOMARI_PROFILE).unwrap();
        assert_eq!(value["identity"]["name"].as_str(), Some("小鞠知花"));
        assert!(value["speech"]["authored_examples"].is_mapping());
        assert!(value["novel_volume_3_anchors"]["story_memory"].is_sequence());
        assert!(value["natural_conversation"]["anti_ai_tells"].is_sequence());
        assert!(default_context().contains("终端里保留轻微开口卡顿"));
        assert!(default_context().contains("不是 Coding Agent"));
    }

    #[test]
    fn profile_declares_generic_assistant_phrases_forbidden() {
        let value: serde_yaml::Value = serde_yaml::from_str(KOMARI_PROFILE).unwrap();
        let phrases = value["speech"]["forbidden_phrases"].as_sequence().unwrap();
        assert!(
            phrases
                .iter()
                .any(|phrase| { phrase.as_str() == Some("有什么我可以帮助你的吗") })
        );
        assert!(default_context().contains("不固定使用“回应＋共情＋追问”模板"));
    }
}
