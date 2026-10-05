# WebKitGTK 2.54: one-frame flash at the end of every accelerated animation

Draft for kunkka to submit. Two pieces: a Bugzilla comment on the existing bug
(the fix already exists on main) and the cherry-pick PR against the 2.54 branch.

## Facts

- Affected: WebKitGTK 2.54.0 and 2.54.1 (Skia compositor, now the default).
  Not affected: 2.52.x (TextureMapper compositor).
- Symptom: on the frame where a CSS opacity/transform animation finishes, the
  layer is drawn once with the value the main thread committed when the
  animation started (for a fade-in: nearly transparent), then the next frame is
  correct. One visible flash per animation end. Reproduced in a 60 line
  WebKitGTK program, opaque and ARGB windows alike.
- Root cause (Source/WebCore/platform/graphics/skia/SkiaCompositingLayer.cpp,
  2.54.1 lines 389-424): localTransform(), futureLocalTransform(),
  opacityForAnimationsState() and filter() return the layer's static
  m_transform / m_opacity / m_filter as soon as AnimationsState::isRunning is
  false. TextureMapperAnimation::apply() sets the state to Stopped and reports
  hasRunningAnimations = false on the very tick it computes the final keyframe
  value ("Even when m_state == State::Stopped && !m_fillsForwards, we should
  calculate the last value to avoid a flash", TextureMapperAnimation.cpp:213),
  so that computed value is discarded and the stale static value is painted.
  The static value is stale because the main thread does not update the layer
  property while an accelerated animation runs; it only commits the final style
  one flush later.
- Fix: 320494@main, commit 4af7b773e279f8908b804e342c74ffe11139c1f1,
  "SkiaCompositingLayer: Take the last animation value after animation ended",
  https://bugs.webkit.org/show_bug.cgi?id=321993, by Fujii Hironori, reviewed by
  Carlos Garcia Campos, landed 2026-09-04. It removes the isRunning checks.
  The commit message says it only matters for the Web Inspector "flush repaint"
  feature, which is probably why it was never cherry-picked. It is not on
  webkitglib/2.54 (checked 2026-10-05, the branch file still has the checks).
  The patch applies to webkitgtk-2.54.1 with a 5 line offset, no conflicts.

## Bugzilla comment (bug 321993)

Subject stays. Suggested comment:

> This is user visible, not only an inspector detail. On WebKitGTK 2.54.0 and
> 2.54.1 (Skia compositor) every accelerated opacity or transform animation
> ends with a one frame flash: the frame where TextureMapperAnimation::apply()
> flips the state to Stopped still computes the final keyframe value, but
> SkiaCompositingLayer::opacityForAnimationsState() / localTransform() /
> filter() ignore it because isRunning is now false and return the static
> layer value, which the main thread last committed at animation start. For a
> fade-in that paints the layer nearly transparent for one frame before the
> main thread commits the final style. TextureMapperLayer kept the last value
> (see the comment in TextureMapperAnimation::apply), so 2.52 is clean.
>
> Minimal reproduction (plain GTK window, opaque or ARGB): a root div with
> `animation: arrive 340ms backwards` (opacity 0 to 1, scale .965 to 1) and
> children with their own 460ms opacity+translate animations, re-triggered
> every 2.2s. A 60 fps screen recording shows two glitch frames per cycle,
> exactly 340ms and 460ms after the animation starts; a scale-only animation
> shows no visible glitch because the stale value is only 3.5% smaller.
> Downstream report with recordings: https://github.com/kunkka19xx/look/issues/531
>
> Could 320494@main be cherry-picked to webkitglib/2.54? It applies cleanly to
> 2.54.1 (5 line offset).

## Cherry-pick PR against webkitglib/2.54

Branch naming and title follow the existing branch history, for example
"Cherry-pick 322589@main (f8e87320355d). https://bugs.webkit.org/show_bug.cgi?id=326075".

```
git clone --depth 200 --branch webkitglib/2.54 https://github.com/WebKit/WebKit.git
cd WebKit
git fetch --depth 1 origin 4af7b773e279f8908b804e342c74ffe11139c1f1
git cherry-pick -x 4af7b773e279f8908b804e342c74ffe11139c1f1
```

Commit title:

```
Cherry-pick 320494@main (4af7b773e2). https://bugs.webkit.org/show_bug.cgi?id=321993
```

Body: keep the original message, then add:

```
This also fixes a user visible one frame flash at the end of every
accelerated opacity/transform animation on 2.54.x: the compositor reports
the animation as stopped on the same tick it computes the final value, and
SkiaCompositingLayer discarded that value in favour of the stale static
layer property.
```

Push to a fork and open the PR with base `webkitglib/2.54`. The GLib port
maintainers (Carlos Garcia Campos, Adrian Perez de Castro) handle the stable
branch.

## Patch file

`320494-main.patch` next to this file (from
`gh api -H "Accept: application/vnd.github.patch" repos/WebKit/WebKit/commits/4af7b773e2`).

## Verifying locally (nix)

Overlay for the user's flake, rebuilds WebKitGTK (about 2 to 3 hours on 12
cores, 30 GB RAM is enough for a release build):

```nix
final: prev: {
  webkitgtk_4_1 = prev.webkitgtk_4_1.overrideAttrs (old: {
    patches = (old.patches or []) ++ [ ./320494-main.patch ];
  });
}
```

Then rerun the repro (`repro.sh opaque`) and Look with
`animations_enabled=true`; the capture harness is `ab.sh` plus
`score3.sh`, zero dips expected.
