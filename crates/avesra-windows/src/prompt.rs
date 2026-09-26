//! Exact-window UIA discovery and guarded one-shot project/draft mutations on
//! the actual native worker. No keyboard input, submit or arbitrary selectors.
use avesra_contracts::ErrorCode;
use avesra_core::{
    apps::AppRecord,
    workflows::{ControlChoice, ControlIdentity, PromptDiscovery},
};
use std::time::{Duration, Instant};
use uuid::Uuid;
use windows::{
    Win32::{
        Foundation::HWND,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::Accessibility::{
            CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
            IUIAutomationTextPattern, IUIAutomationTreeWalker, IUIAutomationValuePattern,
            UIA_InvokePatternId, UIA_TextPatternId, UIA_ValuePatternId,
        },
    },
    core::{BSTR, Interface},
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
fn label(value: BSTR) -> Result<String, ErrorCode> {
    if value.len() > 256 {
        return Err(ErrorCode::TooLarge);
    }
    let value = value.to_string();
    if value.len() > 256 || value.chars().any(char::is_control) {
        return Err(ErrorCode::TooLarge);
    }
    Ok(value)
}
// A successful UIA relationship may return a null element. Preserve the HRESULT
// separately; an actual provider error must not masquerade as an empty subtree.
fn relation(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
    child: bool,
) -> Result<Option<IUIAutomationElement>, ErrorCode> {
    let mut raw = std::ptr::null_mut();
    let status = unsafe {
        if child {
            (Interface::vtable(walker).GetFirstChildElement)(
                Interface::as_raw(walker),
                Interface::as_raw(element),
                &mut raw,
            )
        } else {
            (Interface::vtable(walker).GetNextSiblingElement)(
                Interface::as_raw(walker),
                Interface::as_raw(element),
                &mut raw,
            )
        }
    };
    status.ok().map_err(|_| ErrorCode::Unavailable)?;
    Ok(if raw.is_null() {
        None
    } else {
        Some(unsafe { IUIAutomationElement::from_raw(raw) })
    })
}
pub fn discover(
    record: &AppRecord,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<PromptDiscovery, ErrorCode> {
    scan(record, authorize, |_, controls, complete, _| {
        Ok(PromptDiscovery {
            app: record.id,
            revision: record.revision,
            controls: controls.into_iter().map(|(_, choice)| choice).collect(),
            complete,
        })
    })
}
fn scan<T>(
    record: &AppRecord,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
    consume: impl FnOnce(
        HWND,
        Vec<(IUIAutomationElement, ControlChoice)>,
        bool,
        &mut dyn FnMut() -> Result<(), ErrorCode>,
    ) -> Result<T, ErrorCode>,
) -> Result<T, ErrorCode> {
    let started = Instant::now();
    authorize()?;
    let hints = crate::apps::window_hints(record)?;
    if !hints.complete || hints.windows.len() != 1 {
        return Err(ErrorCode::Denied);
    }
    let hint = &hints.windows[0];
    let check = |authorize: &mut dyn FnMut() -> Result<(), ErrorCode>| -> Result<HWND, ErrorCode> {
        authorize()?;
        if started.elapsed() >= Duration::from_secs(5) {
            return Err(ErrorCode::Expired);
        }
        hint.automation_window(record)
    };
    let window = check(authorize)?;
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|_| ErrorCode::Unavailable)?;
    let _apartment = Apartment;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| ErrorCode::Unavailable)?;
    let root =
        unsafe { automation.ElementFromHandle(window) }.map_err(|_| ErrorCode::Unavailable)?;
    let walker = unsafe { automation.ControlViewWalker() }.map_err(|_| ErrorCode::Unavailable)?;
    let mut stack = vec![(root, 0u8)];
    let mut controls = Vec::new();
    let mut complete = true;
    let mut visited = 0;
    while let Some((element, depth)) = stack.pop() {
        check(authorize)?;
        visited += 1;
        if visited > 128 {
            complete = false;
            break;
        }
        let safe = unsafe {
            element
                .CurrentProcessId()
                .map(|pid| pid > 0 && pid as u32 == hint.pid())
                .unwrap_or(false)
                && !element
                    .CurrentIsPassword()
                    .map(|v| v.as_bool())
                    .unwrap_or(true)
                && !element
                    .CurrentIsOffscreen()
                    .map(|v| v.as_bool())
                    .unwrap_or(true)
                && element
                    .CurrentIsEnabled()
                    .map(|v| v.as_bool())
                    .unwrap_or(false)
        };
        if !safe {
            continue;
        }
        let identity = unsafe {
            ControlIdentity {
                control_type: element
                    .CurrentControlType()
                    .map_err(|_| ErrorCode::Unavailable)?
                    .0,
                name: label(element.CurrentName().map_err(|_| ErrorCode::Unavailable)?)?,
                automation_id: label(
                    element
                        .CurrentAutomationId()
                        .map_err(|_| ErrorCode::Unavailable)?,
                )?,
                class: label(
                    element
                        .CurrentClassName()
                        .map_err(|_| ErrorCode::Unavailable)?,
                )?,
                framework: label(
                    element
                        .CurrentFrameworkId()
                        .map_err(|_| ErrorCode::Unavailable)?,
                )?,
            }
        };
        identity.validate()?;
        check(authorize)?;
        let invoke = unsafe {
            element.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
        }
        .is_ok();
        let writable_value =
            unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
                .ok()
                .and_then(|p| unsafe { p.CurrentIsReadOnly() }.ok())
                .is_some_and(|v| !v.as_bool());
        let readable_text =
            unsafe { element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }
                .is_ok();
        check(authorize)?;
        let claude_route = if identity.control_type == 50030 {
            unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
                .ok()
                .and_then(|pattern| {
                    if !unsafe { pattern.CurrentIsReadOnly() }.ok()?.as_bool() {
                        return None;
                    }
                    let value = unsafe { pattern.CurrentValue() }.ok()?;
                    if value.len() > 1024 {
                        return None;
                    }
                    let value = value.to_string();
                    (avesra_core::workflows::project_route(&value)
                        || avesra_core::workflows::fresh_route(&value))
                    .then_some(value)
                })
        } else {
            None
        };
        controls.push((
            element.clone(),
            ControlChoice {
                id: Uuid::new_v4(),
                identity,
                invoke,
                writable_value,
                readable_text,
                depth,
                claude_route,
            },
        ));
        let mut child = relation(&walker, &element, true)?;
        if child.is_some() && depth >= 12 {
            complete = false;
            continue;
        }
        while let Some(value) = child {
            check(authorize)?;
            if stack.len() + visited >= 128 {
                complete = false;
                break;
            }
            child = relation(&walker, &value, false)?;
            stack.push((value, depth + 1));
        }
    }
    check(authorize)?;
    let mut final_check = || check(authorize).map(|_| ());
    consume(window, controls, complete, &mut final_check)
}

fn supported(record: &AppRecord) -> Result<(), ErrorCode> {
    match &record.launch {
        avesra_core::apps::LaunchIdentity::Packaged {
            app_id,
            package_full_name,
            ..
        } if app_id == "Claude_pzs8sxrjxfjjc!Claude"
            && package_full_name == "Claude_2.9939.2.0_x64__pzs8sxrjxfjjc" =>
        {
            Ok(())
        }
        _ => Err(ErrorCode::Unsupported),
    }
}
pub fn bind(
    resolved: &avesra_core::apps::ResolvedApp,
    app_name: &str,
    actor: Uuid,
    original: PromptDiscovery,
    choices: avesra_core::workflows::BindingChoices,
    authorize: &mut dyn FnMut() -> Result<(), ErrorCode>,
) -> Result<avesra_core::workflows::PromptBinding, ErrorCode> {
    use avesra_core::workflows::PromptBinding;
    supported(&resolved.record)?;
    if !original.complete
        || original.app != resolved.record.id
        || original.revision != resolved.record.revision
    {
        return Err(ErrorCode::Stale);
    }
    let choice = |id| {
        original
            .controls
            .iter()
            .find(|c| c.id == id)
            .ok_or(ErrorCode::Stale)
    };
    let project_button = choice(choices.project_button)?;
    let project_indicator = choice(choices.project_indicator)?;
    let new_chat = choice(choices.new_chat)?;
    let prompt = choice(choices.prompt)?;
    let route = choice(choices.route)?;
    if !project_button.invoke
        || !new_chat.invoke
        || !prompt.writable_value
        || route.identity.control_type != 50030
    {
        return Err(ErrorCode::Unsupported);
    }
    let binding = PromptBinding {
        id: Uuid::new_v4(),
        revision: Uuid::new_v4(),
        actor,
        app: resolved.record.id,
        app_revision: resolved.record.revision,
        alias: resolved.alias_id,
        alias_revision: resolved.alias_revision,
        app_name: avesra_core::apps::alias_phrase(app_name)?,
        project: avesra_core::apps::alias_phrase(&choices.project)?,
        project_route: route.claude_route.clone().ok_or(ErrorCode::Unsupported)?,
        project_button: project_button.identity.clone(),
        project_indicator: project_indicator.identity.clone(),
        new_chat: new_chat.identity.clone(),
        prompt: prompt.identity.clone(),
        route: route.identity.clone(),
    };
    binding.validate()?;
    scan(
        &resolved.record,
        authorize,
        |_, controls, complete, current| {
            if !complete {
                return Err(ErrorCode::Unsupported);
            }
            for control in [
                &binding.project_button,
                &binding.project_indicator,
                &binding.new_chat,
                &binding.prompt,
                &binding.route,
            ] {
                locate(&controls, control, false)?;
            }
            let prompt = locate(&controls, &binding.prompt, false)?;
            let heading = locate(&controls, &binding.project_indicator, false)?;
            if !associated(prompt, heading)? {
                return Err(ErrorCode::Unsupported);
            }
            current()
        },
    )?;
    authorize()?;
    Ok(binding)
}
fn locate<'a>(
    controls: &'a [(IUIAutomationElement, ControlChoice)],
    identity: &ControlIdentity,
    route: bool,
) -> Result<&'a IUIAutomationElement, ErrorCode> {
    let mut matches = controls.iter().filter(|(_, c)| {
        if route {
            c.identity.control_type == identity.control_type
                && c.identity.automation_id == identity.automation_id
                && c.identity.class == identity.class
                && c.identity.framework == identity.framework
        } else {
            c.identity == *identity
        }
    });
    let (element, _) = matches.next().ok_or(ErrorCode::Stale)?;
    if matches.next().is_some() {
        return Err(ErrorCode::Denied);
    }
    Ok(element)
}
fn value(element: &IUIAutomationElement, limit: usize) -> Result<String, ErrorCode> {
    let pattern =
        unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
            .map_err(|_| ErrorCode::Unsupported)?;
    let value = unsafe { pattern.CurrentValue() }.map_err(|_| ErrorCode::Unavailable)?;
    if value.len() > limit {
        return Err(ErrorCode::TooLarge);
    }
    let value = value.to_string();
    if value.len() > limit {
        return Err(ErrorCode::TooLarge);
    }
    Ok(value)
}
fn associated(
    prompt: &IUIAutomationElement,
    heading: &IUIAutomationElement,
) -> Result<bool, ErrorCode> {
    // Preserve successful-null separately from provider failure. A nearby
    // sidebar heading is insufficient; require the actual label relationship.
    let mut raw = std::ptr::null_mut();
    unsafe { (Interface::vtable(prompt).CurrentLabeledBy)(Interface::as_raw(prompt), &mut raw) }
        .ok()
        .map_err(|_| ErrorCode::Unavailable)?;
    if raw.is_null() {
        return Ok(false);
    }
    let label = unsafe { IUIAutomationElement::from_raw(raw) };
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| ErrorCode::Unavailable)?;
    unsafe { automation.CompareElements(&label, heading) }
        .map(|v| v.as_bool())
        .map_err(|_| ErrorCode::Unavailable)
}
pub fn fill(
    record: &AppRecord,
    binding: &avesra_core::workflows::PromptBinding,
    text: &str,
    authority: &mut avesra_core::execution::EffectAuthority<'_>,
) -> Result<avesra_core::execution::EffectResult, ErrorCode> {
    use avesra_contracts::Outcome;
    use avesra_core::execution::{EffectObservation, EffectResult};
    use sha2::{Digest, Sha256};
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    supported(record)?;
    binding.validate()?;
    if record.id != binding.app
        || record.revision != binding.app_revision
        || text.is_empty()
        || text.len() > 16384
        || text.chars().any(char::is_control)
    {
        return Err(ErrorCode::Denied);
    }
    // The pinned package launch/focus path calls this exactly once immediately
    // before its first OS mutation. Later workflow phases recheck the same
    // authority, without consuming or renewing another commit admission.
    let opened = crate::apps::launch(record, &mut || authority.commit())?;
    if opened.outcome != Outcome::Success {
        return Ok(EffectResult {
            outcome: opened.outcome,
            observation: None,
        });
    }
    let started = Instant::now();
    let original = crate::apps::window_hints(record)?;
    if !original.complete || original.windows.len() != 1 {
        return Err(ErrorCode::Denied);
    }
    let original = &original.windows[0];
    let original_window = original.automation_window(record)?;
    let mut authorize = || {
        if started.elapsed() >= Duration::from_secs(15) {
            return Err(ErrorCode::Expired);
        }
        authority.current()?;
        if original.automation_window(record)? != original_window
            || unsafe { GetForegroundWindow() } != original_window
        {
            return Err(ErrorCode::Stale);
        }
        Ok(())
    };
    // Each fresh scan stays on this actual worker. Calls may block, but no waiter
    // timeout releases ownership or retries an Invoke/SetValue operation.
    for phase in 0..4 {
        loop {
            authorize()?;
            let ready = scan(
                record,
                &mut authorize,
                |window, controls, complete, authorize| {
                    authorize()?;
                    if window != original_window {
                        return Err(ErrorCode::Stale);
                    }
                    if !complete {
                        return Err(ErrorCode::Unsupported);
                    }
                    let observed = |identity, route| match locate(&controls, identity, route) {
                        Ok(value) => Ok(Some(value)),
                        Err(ErrorCode::Stale) if phase > 0 => Ok(None),
                        Err(error) => Err(error),
                    };
                    let Some(prompt) = observed(&binding.prompt, false)? else {
                        return Ok(false);
                    };
                    let project = if phase > 0 {
                        let Some(project) = observed(&binding.project_indicator, false)? else {
                            return Ok(false);
                        };
                        Some(project)
                    } else {
                        None
                    };
                    let Some(route) = observed(&binding.route, true)? else {
                        return Ok(false);
                    };
                    let old = value(prompt, 16384)?;
                    if (phase < 3 && !old.is_empty())
                        || (phase == 3 && !old.is_empty() && old != text)
                    {
                        return Err(ErrorCode::Denied);
                    }
                    if phase == 3 && old != text {
                        return Ok(false);
                    }
                    let current_route = value(route, 1024)?;
                    if phase == 1 && current_route != binding.project_route {
                        return Ok(false);
                    }
                    if phase >= 2 && !avesra_core::workflows::fresh_route(&current_route) {
                        return Ok(false);
                    }
                    if let Some(project) = project
                        && label(
                            unsafe { project.CurrentName() }.map_err(|_| ErrorCode::Unavailable)?,
                        )? != binding.project_indicator.name
                    {
                        return Err(ErrorCode::Stale);
                    }
                    if phase >= 2 && !associated(prompt, project.ok_or(ErrorCode::Stale)?)? {
                        return Ok(false);
                    }
                    authorize()?;
                    if unsafe { GetForegroundWindow() } != window {
                        return Err(ErrorCode::Stale);
                    }
                    match phase {
                        0 | 1 => {
                            let Some(target) = observed(
                                if phase == 0 {
                                    &binding.project_button
                                } else {
                                    &binding.new_chat
                                },
                                false,
                            )?
                            else {
                                return Ok(false);
                            };
                            let invoke = unsafe {
                                target.GetCurrentPatternAs::<IUIAutomationInvokePattern>(
                                    UIA_InvokePatternId,
                                )
                            }
                            .map_err(|_| ErrorCode::Unsupported)?;
                            authorize()?;
                            if unsafe { GetForegroundWindow() } != window {
                                return Err(ErrorCode::Stale);
                            }
                            unsafe { invoke.Invoke() }.map_err(|_| ErrorCode::Unavailable)?;
                        }
                        2 => {
                            let pattern = unsafe {
                                prompt.GetCurrentPatternAs::<IUIAutomationValuePattern>(
                                    UIA_ValuePatternId,
                                )
                            }
                            .map_err(|_| ErrorCode::Unsupported)?;
                            if unsafe { pattern.CurrentIsReadOnly() }
                                .map_err(|_| ErrorCode::Unavailable)?
                                .as_bool()
                            {
                                return Err(ErrorCode::Denied);
                            }
                            authorize()?;
                            if unsafe { GetForegroundWindow() } != window
                                || !value(prompt, 16384)?.is_empty()
                                || !associated(prompt, project.ok_or(ErrorCode::Stale)?)?
                            {
                                return Err(ErrorCode::Stale);
                            }
                            unsafe { pattern.SetValue(&BSTR::from(text)) }
                                .map_err(|_| ErrorCode::Unavailable)?;
                        }
                        _ => {}
                    }
                    authorize()?;
                    Ok(true)
                },
            )?;
            if ready {
                break;
            }
            // Only a read-only unmet postcondition reaches here. Any error after a
            // mutation exits via `?`, so Invoke and SetValue are never retried.
            authorize()?;
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    authorize()?;
    Ok(EffectResult {
        outcome: Outcome::Success,
        observation: Some(EffectObservation::PromptDraft {
            app_id: record.id,
            project_id: binding.id,
            binding_revision: binding.revision,
            text_sha256: Sha256::digest(text.as_bytes()).into(),
            fresh_composer: true,
            submitted: false,
        }),
    })
}
