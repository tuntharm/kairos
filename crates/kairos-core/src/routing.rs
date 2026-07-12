use serde::Serialize;

use crate::{BrainRecord, CoreError, KairosConfig, Result};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutedBrain {
    pub id: String,
    pub name: String,
    pub score: u32,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteResult {
    pub query: String,
    pub brains: Vec<RoutedBrain>,
    pub requires_choice: bool,
    pub unavailable_brains: Vec<String>,
}

const PHD_HINTS: &[&str] = &[
    "phd",
    "abaqus",
    "beam",
    "plate",
    "surrogate",
    "finite element",
    "experiment",
    "supervision",
    "literature",
    "cnn",
];
const DATTER_HINTS: &[&str] = &["datter", "token waste", "data usefulness", "compression"];
const BORDER_HINTS: &[&str] = &["border fiber", "fiber ids", "phi-otdr", "das"];
const BRIEF_HINTS: &[&str] = &[
    "brief me",
    "what should i do",
    "what matters today",
    "next action",
    "today",
];

/// Internal query used only by the What Next dashboard. It deliberately
/// selects the Everyday and PhD context together when both are available;
/// ordinary mixed questions continue to avoid broadcasting into both brains.
pub const CROSS_BRAIN_PULSE_QUERY: &str =
    "kairos cross-brain pulse: personal everyday life and phd research priorities";

fn keyword_score(query: &str, keywords: &[&str]) -> u32 {
    keywords
        .iter()
        .filter(|keyword| query.contains(**keyword))
        .count() as u32
}

fn score(brain: &BrainRecord, query: &str) -> u32 {
    let configured_score = brain
        .routing_hints
        .iter()
        .map(|hint| hint.trim().to_lowercase())
        .filter(|hint| hint.len() >= 2 && query.contains(hint.as_str()))
        .count() as u32;
    let legacy_score = match brain.id.as_str() {
        "phd" => keyword_score(query, PHD_HINTS),
        "datter" => keyword_score(query, DATTER_HINTS),
        "border-fiber" => keyword_score(query, BORDER_HINTS),
        "everyday" if BRIEF_HINTS.iter().any(|hint| query.contains(hint)) => 4,
        "everyday" => 1,
        _ => 0,
    };
    // Human-configured hints are deliberate routing metadata and take
    // precedence over the historic ID-specific vocabulary.
    configured_score.saturating_mul(5).max(legacy_score)
}

pub fn route_query(
    config: &KairosConfig,
    query: &str,
    brain_override: Option<&str>,
) -> Result<RouteResult> {
    let query = query.trim();
    if query.is_empty() {
        return Err(CoreError::InvalidPath("query must not be empty".to_owned()));
    }
    let lower = query.to_lowercase();
    let is_cross_brain_pulse = lower == CROSS_BRAIN_PULSE_QUERY;
    if let Some(override_id) = brain_override {
        let brain = config
            .brains
            .iter()
            .find(|brain| brain.id == override_id)
            .ok_or_else(|| CoreError::BrainNotFound(override_id.to_owned()))?;
        if !brain.enabled {
            return Err(CoreError::BrainDisabled(override_id.to_owned()));
        }
        return Ok(RouteResult {
            query: query.to_owned(),
            brains: vec![RoutedBrain {
                id: brain.id.clone(),
                name: brain.name.clone(),
                score: 100,
                reason: "manual brain override".to_owned(),
            }],
            requires_choice: false,
            unavailable_brains: Vec::new(),
        });
    }

    let mut unavailable_brains = Vec::new();
    let mut candidates: Vec<(u32, &BrainRecord)> = config
        .brains
        .iter()
        .filter_map(|brain| {
            let score = score(brain, &lower);
            if score == 0 {
                return None;
            }
            if !brain.enabled {
                unavailable_brains.push(brain.id.clone());
                return None;
            }
            Some((score, brain))
        })
        .collect();

    // Everyday is the helpful fallback, not automatic extra context for a
    // clearly-owned specialist question such as Abaqus/PhD work.
    if !is_cross_brain_pulse
        && candidates
            .iter()
            .any(|(score, brain)| brain.id != "everyday" && *score > 0)
    {
        candidates.retain(|(_, brain)| brain.id != "everyday");
    }

    if candidates.is_empty() {
        let fallback = config
            .brains
            .iter()
            .find(|brain| brain.id == "everyday" && brain.enabled)
            .or_else(|| config.brains.iter().find(|brain| brain.enabled))
            .ok_or(CoreError::NoEnabledBrain)?;
        candidates.push((1, fallback));
    }

    candidates.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.id.cmp(&right.id))
    });
    candidates.truncate(2);

    let requires_choice =
        !is_cross_brain_pulse && candidates.len() > 1 && candidates[0].0 == candidates[1].0;
    let brains = candidates
        .into_iter()
        .map(|(score, brain)| RoutedBrain {
            id: brain.id.clone(),
            name: brain.name.clone(),
            score,
            reason: if is_cross_brain_pulse {
                "selected by Kairos's default cross-brain pulse".to_owned()
            } else if brain.id == "everyday" && score == 1 {
                "default life-admin route; choose another brain if this is wrong".to_owned()
            } else {
                "matched Kairos routing vocabulary".to_owned()
            },
        })
        .collect();

    Ok(RouteResult {
        query: query.to_owned(),
        brains,
        requires_choice,
        unavailable_brains,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::default_tharm_config;

    #[test]
    fn phd_language_routes_to_phd() {
        let config = default_tharm_config().unwrap();
        let result =
            route_query(&config, "What Abaqus plate experiment comes next?", None).unwrap();
        assert_eq!(result.brains.first().unwrap().id, "phd");
        assert_eq!(result.brains.len(), 1);
    }

    #[test]
    fn morning_question_stays_in_everyday_context() {
        let config = default_tharm_config().unwrap();
        let result = route_query(&config, "What should I do today?", None).unwrap();
        assert_eq!(result.brains.first().unwrap().id, "everyday");
    }

    #[test]
    fn cross_brain_pulse_intentionally_uses_everyday_and_phd_without_ambiguity() {
        let config = default_tharm_config().unwrap();
        let result = route_query(&config, CROSS_BRAIN_PULSE_QUERY, None).unwrap();
        let ids = result
            .brains
            .iter()
            .map(|brain| brain.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["everyday", "phd"]);
        assert!(!result.requires_choice);
    }

    #[test]
    fn registered_routing_hints_support_new_brains_without_hard_coding() {
        let mut config = default_tharm_config().unwrap();
        let mut brain = config.brains[0].clone();
        brain.id = "creative".to_owned();
        brain.name = "Creative Brain".to_owned();
        brain.routing_hints = vec!["songwriting".to_owned(), "music practice".to_owned()];
        config.brains.push(brain);
        let result = route_query(&config, "Help me plan my songwriting practice", None).unwrap();
        assert_eq!(result.brains.first().unwrap().id, "creative");
    }
}
