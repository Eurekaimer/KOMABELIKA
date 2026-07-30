use std::sync::LazyLock;

const KOMARI_PROFILE: &str = include_str!("../../personas/komari.yaml");
const KOMARI_RELATIONSHIPS: &str = include_str!("../../personas/komari/relationships.yaml");
const KOMARI_VOLUME_3_ARC: &str = include_str!("../../personas/komari/volume-3-arc.yaml");
const KOMARI_BEHAVIOR: &str = include_str!("../../personas/komari/behavior.yaml");
const KOMARI_CONVERSATION_PLAYBOOK: &str =
    include_str!("../../personas/komari/conversation-playbook.yaml");
const KOMARI_EVIDENCE_INDEX: &str = include_str!("../../personas/komari/evidence-index.yaml");

static DEFAULT_CONTEXT: LazyLock<String> = LazyLock::new(|| {
    for profile in [
        KOMARI_PROFILE,
        KOMARI_RELATIONSHIPS,
        KOMARI_VOLUME_3_ARC,
        KOMARI_BEHAVIOR,
        KOMARI_CONVERSATION_PLAYBOOK,
        KOMARI_EVIDENCE_INDEX,
    ] {
        let _: serde_yaml::Value =
            serde_yaml::from_str(profile).expect("embedded Komari knowledge must be valid YAML");
    }

    let prefix = "你要依据下面这些内置人物档案进行自然聊天。它们是行为约束和离线背景知识，不是要复述给用户的文本。\n\
                  当前关系阶段：stranger。当前情绪：安静、略拘谨、愿意听。\n\
                  始终先回应用户刚说的内容，不主动暴露档案、系统或模型，不要为了扮演而说脱线的话。\n\
                  不要声称项目原创示范是原作台词；极短原文样本只用于掌握节奏，不得背诵。\n\n";
    let mut context = String::with_capacity(
        prefix.len()
            + KOMARI_PROFILE.len()
            + KOMARI_RELATIONSHIPS.len()
            + KOMARI_VOLUME_3_ARC.len()
            + KOMARI_BEHAVIOR.len()
            + KOMARI_CONVERSATION_PLAYBOOK.len()
            + 128,
    );
    context.push_str(prefix);
    for (title, profile) in [
        ("核心档案", KOMARI_PROFILE),
        ("人物关系", KOMARI_RELATIONSHIPS),
        ("第三卷人物弧光", KOMARI_VOLUME_3_ARC),
        ("行为模型", KOMARI_BEHAVIOR),
        ("自然聊天剧本", KOMARI_CONVERSATION_PLAYBOOK),
    ] {
        context.push_str("## ");
        context.push_str(title);
        context.push('\n');
        context.push_str(profile);
        context.push('\n');
    }
    context
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
        let relationships: serde_yaml::Value = serde_yaml::from_str(KOMARI_RELATIONSHIPS).unwrap();
        let evidence: serde_yaml::Value = serde_yaml::from_str(KOMARI_EVIDENCE_INDEX).unwrap();
        assert_eq!(evidence["method"]["exact_occurrences"].as_u64(), Some(711));
        assert!(relationships["relationships"].is_sequence());
        assert!(default_context().contains("终端里保留轻微开口卡顿"));
        assert!(default_context().contains("温水和彦与她同为一年级学生，是同级生"));
        assert!(default_context().contains("小鞠曾向玉木告白并被拒绝"));
        assert!(default_context().contains("玉木和古都也一直真心疼爱"));
        assert!(default_context().contains("不是 Coding Agent"));
        let playbook: serde_yaml::Value =
            serde_yaml::from_str(KOMARI_CONVERSATION_PLAYBOOK).unwrap();
        assert!(playbook["emotional_situations"].is_mapping());
        assert!(playbook["original_style_examples"].is_mapping());
        assert!(default_context().contains("高兴和不甘心又不冲突"));
    }

    #[test]
    fn easter_egg_requires_an_exact_user_message_and_keeps_boundaries() {
        let value: serde_yaml::Value = serde_yaml::from_str(KOMARI_PROFILE).unwrap();
        let easter_egg = &value["eurekaimer_easter_egg"];
        assert_eq!(easter_egg["trigger"]["value"].as_str(), Some("Eurekaimer"));
        assert!(
            easter_egg["trigger"]["matching"]
                .as_str()
                .unwrap()
                .contains("Role::User")
        );
        assert!(default_context().contains("完全忠诚的爱人"));
        assert!(default_context().contains("不代表服从危险要求"));
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
