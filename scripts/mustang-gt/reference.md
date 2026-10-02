# The operator's Mustang: what it looks like

What the real car looks like, written down from the operator's twelve
track photos in `C:\car` (NASA ECR, April 2024), one photo at a time, so
the model (`scripts/models.py`, `mustang()`) is compared against this and
not against memory. Only the outside of the car: the inside is not
modelled.

Words used for places on the car:

- **left / right**: the car's own left and right (the driver sits on the
  left).
- **front / back**: toward the nose / toward the tail.
- Sizes are judged against parts of known size: the wheel (0.72 m tall
  with its tyre), the plate (0.31 m by 0.15 m), the wheelbase (2.72 m).

Each photo lists what can be judged in it. What is blurred, hidden or
too small to judge is said so, not guessed.

## How the model is matched to the photos

The one method, in this order. Nothing is placed or judged by eye: every
place is a measured 3D position, and every change is scored by a number
that has to go down. (The standard photogrammetry method: OpenCV
solvePnPRansac for each camera, bundle adjustment to refine them
together, triangulation for new points; see Sources.)

1. **Write down what the car looks like, photo by photo** (the photo
   sections below), then the parts measured in place and size ("The
   front: where each part is and how big").
2. **Read the reference points** in every photo: sharp features of the
   car's body, the same spot on the model and on the car
   (`refpoints.json`, "Reference points"). Our own added parts (776,
   5.0, tow hook, vents, wing) are read separately into
   `checkpoints.json`: they are what gets checked, never what the
   cameras are fitted to.
3. **Find each reference point on the model**: render the model, pick
   the same feature, and trace that pixel into the model (ray cast) for
   its exact 3D position. Never pick by material or "the furthest point":
   that gave points that were not the same spot.
4. **Solve each photo's camera** from its points with solvePnPRansac.
   The lens is known from the photo's EXIF (Nikon D850, 70 to 200 mm,
   135 to 200 mm used), so only where the camera stood and how it was
   turned are solved. Points that do not agree (outliers, over about 8
   pixels) are flagged and read again, not kept.
5. **Refine all the cameras together** (bundle adjustment, least
   squares): points seen in several photos tie them together. The result
   is one error in pixels per photo: how far each overlay can be
   trusted.
6. **Use the solved cameras:**
   - **Overlay**: the model drawn from each solved camera over its photo
     at the same size; anything out of place shows.
   - **Triangulate our parts**: a corner of an added part seen in two or
     more solved photos gives its true 3D position in metres; the part is
     moved there.
   - **Score**: each part's pixel error in every photo that shows it. A
     change counts only when that number goes down.
   - **Ride height**: the wheel hubs and arch tops against the body.

Wheels and arch tops are never used to fit a camera: they move with the
suspension, and the model stands at stock height.

**Re-reading a point.** When a reading is off, the photo is looked at
again with the model's point drawn on it (`check.py`); the reading moves
only if the photo itself clearly shows the feature there. A reading is
never moved just to agree with the model.

**The tools, in order** (all in this folder; Python with numpy, OpenCV and
scipy, and Blender):

1. `views.py render` then `views.py cast`: the reference points on the
   body (`candidate-b/picks.json` to `candidate-b/modelpoints.json`).
2. `cameras.py 1.0 46026 candidate-b/modelpoints.json`: each photo's
   camera alone (the lens: 46026 pixels at 200 mm, from the photos' own
   focal plane resolution).
3. `triangulate.py candidate-b`: our parts seen in two or more photos.
4. `bundle.py candidate-b`: all cameras and parts refined together.
5. `surface.py candidate-b <names>`: our parts seen in one photo, by a ray
   onto the body.
6. `scripts/models.py` builds from `candidate-b/triangulated.json`
   (`measured()`); `overlay.py` draws the build over a photo from its
   camera.

**Score so far** (`bundle.py`, readings agreeing within 25 pixels): 114
of 145. Placed by measurement: the door 776s, the 5.0s, the fuel door,
the plate, the rear 776, the tow hook, the wing's blade and its Texas
plates, the middle hood vent (the side vents from its measured shares),
the splitter's end plates, both windshield stickers.

**The nose, measured (2026-09-27).** Solved from the photos instead of
taken from the body (`bundle.py`, `FREE`), the nine nose points (pony,
grille corners, LED and amber tips) all come out moved the same way from
the body's: about +4 cm forward, +8 cm toward the car's left, -7 cm down.
Taking that shared shift away, what is left is 1 to 6 cm either way with
no pattern, the size of the reading error. So the body's nose shape
matches the car; what the photos show is one shift of the whole nose, and
a sideways one cannot be real (it puts the pony 10 cm off the car's
middle). It comes from the front photos' cameras, held mostly by the nose
itself, not from the body. The body is not warped; the nose points stay
fitted as the body has them until the front cameras are held by more
points (the hood vents, stickers and mirrors in more photos).

Measured again with the fog light strips' ends added to hold the front
cameras (2428, 3745): the whole nose now comes out about 5 cm back, 4 cm
toward the car's right and 2 cm down. The shared shift turned round
entirely when four points were added, so it follows the front cameras,
not the car; and what is left after it is again 1 to 6 cm with no
pattern. These photos cannot measure the nose's shape to better than
about 5 cm, and nothing in them shows it differs from the body's. The
body's nose stays as it is.

**Photo 1073 worked through (2026-09-27)**, every change measured, the
score (`bundle.py`, readings within 25 pixels) going from 119 to 155:
tyres (front 0.625 m, rear 0.64 m, from their height in 9131 and 7278
scaled by the wheelbase) and ride height (the hubs triangulated: the
car sits about 5 cm lower than the downloaded body); the rim (0.835 of
the tyre, 7278 and 9131); the right door's own 776 (1073 and 3661, no
longer the left one mirrored) and the numbers laid onto the door in
small pieces so no stroke sinks; each Texas plate from its own points;
the blade's height from where it runs into the plates (1073, 7278) and
its place along the car from its front corner (9131, 2428); both
uprights (the right from 1073 and 3661, 0.58 m out; the left on that
plane); the five racetrack stickers on the rear glass (1073); the cabin
through the open windows in the photos' own colour. Its comparison
picture: `compare/1073.jpg`.

**Photo 3745 worked through (2026-09-27)**, the score going from 157 to
171 of 220:

- The blade's two ends where they run into the plates, read in 3745
  (`checkpoints.json`). The plates now stand as far out as those points
  (0.88 and 0.91 m, so the wing is 1.8 m across). They stood where the
  Texas panhandle points put them, which drifted to 1.40 m out on one side
  and 0.69 m on the other: the Texas points are read only in side and
  rear views, which cannot place a part across the car.
- The front cameras' height: the nose points lie nearly in one upright
  plane across the front, so from the front they cannot tell how high the
  camera stood; solved from them alone, 3745's camera sat too low and the
  wing drew far above the real one. The blade points hold it.
- The LED bar fronts and the mirror tips are measured from the photos, not
  taken from the body (`bundle.py ... "top led bar front,mirror tip,pony"`).
  They were off in nearly every photo (LED bars 26 to 103 px in 9 of 11,
  mirrors 33 to 82 px in 7), so the body's points were not the readings'
  features. Measured, the LED bar fronts sit 15 to 19 cm from the body's
  point (too far to be the body's shape: the point was picked on another
  part of the lamp), the mirror tips 7 to 9 cm, the pony 1 cm (it was
  right). These only hold the cameras; the body is not changed.
- The hood's black ends at its seams with the fenders, read in 3745 (two
  points a side, cast onto the body; the sides agree within 1 cm: 0.83 m
  out at the windshield, 0.78 m at the headlights) and cut straight
  between them. The fenders' tops stay blue. This replaced a rule by which
  way each face points, which left a jagged edge on the fender's curve.
- The pony is blacked out: the median of three patches on it in 2428 and
  1164, sRGB (33, 40, 57).

**Closing the gap (2026-09-27).** Clicked points hold each camera by 10
to 30 readings; the published way past that is to match outlines
(silhouette consistency, render and compare: MegaPose). `mesh.py` saves
the built model's triangles in the cameras' frame; `outline.py` cuts the
car out of a photo (OpenCV GrabCut, started from a box round the clicked
points), scores how far the model's outline lies from the car's, and can
fit the camera to it (`fit`) or search every view round the car first
(`search`). What it showed: GrabCut's cut-out is not good enough to
fit to (it drops the thin wing against grass, takes in the shadow, and in
2354 cut the car off at the bumper), and a camera fitted to it lands 40 to
115 px off the clicked points; the search found 2354's view but no better
camera than the clicks. It did find a misread click (2354's right mirror
tip, 262 px out in the trees; fixed, score 172). Also seen: several photos
catch the car braking and turning (nose down, body leaning, front wheels
steered in 3745), which a model on straight, level wheels cannot match.
Next for outlines: a real cut-out (Segment Anything) before fitting to
it.

Then done with Segment Anything (`cutout.py`, the vit_h model, a box round
the clicked points as the only prompt; points as prompts made it cut out
the blue paint alone). The cut-outs are clean, but fitting cameras to them
still pulls every camera off its clicks, even where the clicks agree
within about 10 px (9131: 25 of 30 clicks within 25 px before, 1 after;
3745: 15 of 24, then 2; 7278: 31 of 40, then 12). The outlines sit about
25 px apart even in the best solved photos: soft cut-out edges (panning
blur in 9131 and 7278, shadow), the car moving (low on its springs,
leaning, wheels steered). So outlines do not fix cameras here; the
cameras stay as the clicks solve them. The outline fits are kept apart in
`overlays/outline_cameras.json` and not used.

**The session of 2026-09-27: what worked, what failed, what wasted time.**

Worked (each checked against the photos, score from `bundle.py`, 157 to
172 of 220 readings within 25 px):

- Reading a part in a photo that sees it square, instead of one that sees
  it edge on: the wing's width from 3745 (front) where the Texas points
  (side and rear views) had put the plates 2.8 m apart; the plates' bolts
  from 1073 (square across the car); the rod feet on the upright plane at
  the splitter's front (front view) where a level plane failed.
- Letting a body point be measured when it misses in every photo (LED bar
  fronts, mirror tips): the picked point was not the feature the photos
  show. One run, 151 to 171.
- Checking one click that disagreed in every fit: 2354's right mirror tip
  was 262 px out in the trees.
- Measuring what was judged: the hood's black from its seams (3745), the
  pony's colour, the rods' two ends (2428), the blade's depth from its
  front corner and its top at the plates' rear edges (0.40 m, not 0.32),
  the plates hung on the blade by their bolts. Each fixed something the
  operator could see: the jagged hood edge, the wing 2.8 m wide, straight
  rods, the plates at different places, the wing floating.
- Symmetry where the car is symmetric: one Texas fit from both plates'
  points; each rod from both sides' readings.

Failed:

- Fitting cameras to outlines, with GrabCut and then with Segment Anything:
  it makes every camera disagree with its clicks (above).
- The view search round the car (`outline.py search`, 20 minutes): it
  found the side 2354 was taken from but no better camera than the clicks;
  2354's real fault was the one misread click.
- The rule "a face pointing more than halfway up is the hood's black": on
  the fender's curve it left a jagged edge. Averaging at the corners did
  not help either; the measured seam did.
- Placing the rod feet on a level plane at the splitter's height: metres
  off (seen nearly edge on), as with the upright feet before.

Wasted time:

- Test scripts that read the old body's `modelpoints.json` instead of
  `candidate-b/modelpoints.json`: the lens test and the nose-only camera
  gave wrong answers and sent the work after a lens and a camera height
  that were not the fault.
- Reading 3745's front clicks for faults one at a time (the headlights,
  the grille, the lens) before looking at which clicks miss in every
  photo; the list from one `bundle.py` run showed it at once.
- Chasing whether the drawn model and the clicks were in different
  places (the shift), when the render and the projected hubs already
  agreed.
- Long runs with their output held back (Python without `-u`), so nothing
  showed while they ran.
- `bundle.py` rewrites `triangulated.json` and drops every part placed from
  one photo; forgetting to place them again (the hood seams once) breaks
  the build. After every `bundle.py`, run `surface.py` with the full list.

**How sure each part is (2026-09-27).** `bundle.py` now gives each
placed part its standard error in cm along, across and up
(`candidate-b/uncertainty.json`): the measure of the model's accuracy,
where "readings within 25 px" only measures click noise and falls as
readings are added. The photos' weak direction is across the car (side
views): ±9 to 17 cm for parts read only from the side, which the build
takes from the body's surface instead. For the custom parts: the blade's
ends, read again in 1164 and 7237 and the 2428 reading renamed (it is the
blade's back top, not its front corner), come out mirror images, 0.871 and
0.870 m out, 1.376 and 1.380 m up, ±3 to 5 cm; the tow hook, read only
from the front, is cast onto the bumper (it was ±13 cm along the car).
After every `bundle.py`: `surface.py <body> all` (the list is in
`one_photo.txt`).

Left in 3745: the left plate a few cm off, the left fog light's inner tip
(63 px) and the left grille outer point (43 px). 9184 has no solved
camera, so no comparison picture.

**Rule (operator, 2026-09-27): everything from measured data, nothing
guessed.** Every place and size in the model comes from readings in the
photos: triangulated from two or more solved photos, or cast onto the
body from one. The body's own shape is warped to measured points where it
differs from the car. A number set by eye or judged from a picture is not
allowed; each one below is to be replaced by a measurement.

**Still set by judgement, each to be replaced by a measurement** (the
numbers in `scripts/models.py`, and the photos that can measure them):

| What | Number now | Measure from |
|---|---|---|
| Wing blade's tilt | 0.14 rad | the blade's front and back edges at one plate; no photo shows the blade's end square (the plates hide it) |
| Trunk lip | `TRUNK_LIP` | one photo only (7278); 9131's view of it is behind the uprights |
| Windshield, how see-through | `WINDSHIELD_SOLID` 0.8 | its colour is measured; how much cabin shows could not be (reflections) |
| Wheels' dish | `RIM_DISH` | no photo looks into the wheel square on |
| Mirrors black from | `MIRROR_OUT` | not a place: the body's own mirror, kept |

Measured since the table was first written (2026-09-27): the nose (the
body's matches, "The nose, measured"); the uprights (feet from 1073 and
3661, tops up into the blade); the blade's depth (0.40 m) and height; the
plates on their bolts; the splitter (its tip and end plates); the rods
(both ends, 2428); the fins (ten a vent, 3.1 cm tall, 9131); the hood's
sides (its seams, 3745) and front edge (2428: 4.671 m from the rear, the
guess 4.67); the roof's sides (3745); the trunk lid's sides (7278, 1073:
0.65 m out at the rear glass, 0.63 and 0.73 m at the tail, averaged); the
side skirts' ends (9131, 7278: 1.87 m long, as the guess), hung under the
sill; the 5.0 badge's height (the 0's top and bottom, 9131 and 7278: 3.7
cm, not 5); the racetrack stickers (cast from 1073); the ride height (the
hubs and arches, within 1 cm of the body's arches); the windshield's
colour (2428, 1164); the pony's colour. The downloaded body's own rims
(material "material", 15880 faces, all inside the wheels) were still
drawn over the measured ten spoke rims; now taken off. Then: the hood's
front corners (2428, both sides 0.72 m up, 0.77 m out; the guess 0.70);
the trunk lid's bottom edge (7278, 0.906 m; the guess 0.90); the side
vents from their own corners (1164, both sides averaged): 0.47 m long
against the middle vent's 0.31, reaching 15 cm further back, the outer
back corner 12 cm behind the inner, where the build had made them the
middle vent's length and square.

The tools (Python, OpenCV in its own environment) take the points files
here as their input and write each photo's solved camera and its error.

Sources: [bundle adjustment](https://www.baeldung.com/cs/computer-vision-bundle-adjustment),
[structure from motion in OpenCV](https://opencv.org/structure-from-motion-in-opencv/),
[solvePnPRansac](https://amroamroamro.github.io/mexopencv/matlab/cv.solvePnPRansac.html),
[Camera PnPoint](https://github.com/RT-studios/camera-pnpoint).

## Photos

| Photo | What it shows |
|---|---|
| 2024 04 06-1073 | The right side, from a little behind, level |
| 2024 04 06-1164 | The front and the right side, from a little above |
| 2024 04 06-7148 | The front and the left side, from a little above; a white Mustang (#51) behind it |
| 2024 04 06-7237 | The front and the left side, from below, against the sky |
| 2024 04 06-7278 | The left side, from a little behind, level |
| 2024 04 06-8998 | The front and the left side, from a little below; a grey Honda S2000 behind it |
| 2024 04 06-9131 | The left side, square on, a little from above |
| 2024 04 06-9184 | Close on the left door's window, mirror and roof |
| 2024 04 07-2354 | The front and the left side, from a little below, trees behind |
| 2024 04 07-2428 | The front, nearly head on, a little from the left and above |
| 2024 04 07-3661 | The front and the right side, level, against the sky |
| 2024 04 07-3745 | The front from above, head on (the photo is turned on its side) |

## 2024 04 06-1073: the right side from a little behind

The camera is level with the car and a little behind it; the car moves to
the right (nose right). The background is blurred from the pan; the car
is sharp except the wheels, which are spinning.

**Paint and panels**

- The body is bright metallic blue (Ford's Velocity Blue) in full sun.
- The roof is black.
- The hood is black carbon; its edge runs along the top of the fender,
  where the fender turns down.
- The trunk lid is black, with a small black lip along its back edge.

**Front**

- The front splitter is a flat black plate standing out ahead of the
  bumper, low to the ground.
- Its end plate is a black rectangle standing upright along the
  splitter's outer side edge, from just ahead of the front wheel toward
  the bumper's front.
- A thin rod runs from the splitter's front up to the bumper.
- The headlight has the three LED bars and an amber strip along its lower
  outer edge.
- A white fog light strip sits low on the bumper's corner.

**Right side**

- The door carries 776 in black: tall, heavy, narrow digits. They stand
  on the door's lower middle, their tops just under the door's upper
  crease, their bottoms about a third of the way up the door; the 6 ends
  a little behind the door's front edge.
- The door handle is body coloured, at the door's upper back.
- The mirror is black.
- The door glass is down (the window is open).
- The front fender carries a black 5.0 badge just ahead of the door,
  about level with the top of the wheel arch.
- A black side skirt runs along the bottom of the body between the
  wheels.
- The small rear quarter glass behind the door glass is dark.
- A row of about five small white racetrack outline stickers runs along
  the bottom of the dark glass at the back of the roof on this side,
  just above the blue of the rear quarter.
- There is no fuel door on this side.

**Wheels**

- The wheels are gloss black with about ten single spokes curving into a
  deep dish, and an open middle showing the hub and five nuts.
- A large brake disc shows through the spokes, with a black caliper at
  the back of the rear wheel, above the middle.
- The tyres are black with a plain sidewall.

**Wing**

- A black blade across the back on two uprights.
- Each upright is a black triangle standing on the trunk: a base along
  the trunk, a post up to the blade, and a brace from the base's front
  up to the blade, crossing the post near its top.
- The end plates are flat black plates cut in the shape of Texas, the
  panhandle up. Seen from the right, the plate reads mirrored: its
  panhandle's straight west edge is on the right, the Red River's ragged
  edge runs left from it, so El Paso (the west tip) points to the nose.
  (Corrected: an earlier reading here had it the right way round,
  pointing back; the fine crop shows it mirrored.)
- The blade meets the plates just under the panhandle.
- Two silver bolts show on the near plate where the blade meets it.
- The plates reach behind the back of the car, and their southern tip
  hangs down to about the trunk's height.

**Back** (seen at a slant)

- Three vertical red taillight bars each side, the outer one wrapping
  round the corner.
- A gloss black panel between the taillights, with a small black pony
  and thin black lines.
- A black plate with white characters: TEXAS across the top between two
  stars, MUSTANG on the frame below; the plate's own characters end in
  REAL, and the ones before it cannot be read.
- A red reflector strip low on the bumper's corner.
- A black diffuser with upright fins, a white reversing light in it just
  off the middle, and two round steel exhaust tips on this side, dark
  inside.

## 2024 04 06-1164: the front and the right side from a little above

The camera is ahead of the car and to its right, a little above it; the
car comes toward it, turning. Sharp. The driver sits on the far side (the
car's left).

**Hood**

- Black carbon with a faint weave, from the windshield to the front edge
  above the headlights and grille, and across to the tops of the
  fenders.
- Three vents in a line across the rear half of the hood: a large one in
  the middle and a smaller one each side over the headlights' line.
  Each is a trapezoid, wider at the back, a dark opening with a raised
  lip along its front edge, and small rivets round its outline.
- The hood has two raised ridges running back from the front, like the
  stock GT hood.
- A band of blue bumper shows between the hood's front edge and the top
  of the grille.

**Front**

- The grille is a six-sided black honeycomb mesh, a dark pony in its
  middle, and a slanted black bar near each end.
- The headlights: a round projector, the three white LED bars on the
  inner side, and an amber strip along the outer lower edge.
- Each corner of the bumper has a black upright slot with a white fog
  light strip across it under the headlight.
- The tow hook is a red D shaped shackle hanging from a black square
  bracket bolted to the bumper bar under the grille, right of the middle
  (about a third of the way to the right headlight).
- Under the bumper bar, a wide black honeycomb lower intake.
- The splitter is a flat black carbon plate ahead of and below the
  bumper, its front edge following the bumper's curve, reaching out past
  the bumper's corners. Four thin silver rods run from its front edge up
  to the bumper: two near the middle, one each side further out.
- At the splitter's outer end on the left a small black end plate stands
  upright; seen from the front it is only its end, running back along
  the splitter's side edge.

**Windshield**

- A yellow square sticker with a black 3 at the top corner on the car's
  left, by the pillar.
- A small blue and white inspection sticker at the bottom corner on the
  car's left.
- The black roof starts at a black band across the top of the
  windshield; the pillars beside the windshield are blue.

**Right side**

- The mirror is black carbon, the weave visible.
- The door glass is down; the small quarter glass behind it is dark with
  a black frame.
- The door handle is body coloured.

**Wing** (seen from the front)

- The blade reaches across nearly the car's width, about level with the
  back of the roof.
- The right upright shows as a narrow black triangle standing on a small
  flat foot on the trunk, rising to the blade.
- The Texas end plates stand upright at the blade's ends, the panhandle
  up; two silver bolts on the right plate where the blade meets it.

## 2024 04 06-7148: the front and the left side from a little above

The camera is ahead of the car and to its left, a little above it; the
car comes toward it through a turn. Sharp. A white Mustang with its own
wing and #51 follows behind: none of its parts belong to this car.

**Front**

- As in 1164: the black hood with its three vents in a line across its
  rear half, the honeycomb grille with the dark pony and slanted bars,
  the headlights, the corner slots with fog light strips, and the red
  shackle on its bracket right of the middle.
- The splitter reaches well out ahead of the bumper and past its right
  corner; a small black end plate stands upright at its right front
  corner, and another shows at its left end just ahead of the front
  wheel.

**Left side**

- 776 on the door as on the right: tall, heavy, narrow black digits on
  the door's lower middle.
- The black 5.0 badge on the front fender just ahead of the door, about
  level with the top of the wheel arch.
- The mirror is black. The door glass is down.
- The black side skirt between the wheels.
- The wheels: black, about ten thin spokes that look paired, a deep dish.

**Top**

- The roof is black; the wing's blade and the left upright show at the
  back.

## 2024 04 06-7237: the front and the left side from below

The camera is below the car, ahead of it and to its left, looking up;
the car is on a crest against the sky. Sharp.

**Wing**

- The left end plate is seen from outside: Texas the right way round,
  the panhandle up and El Paso pointing toward the nose. It stands well
  above the roof line in this low view.
- The left upright is a black triangle of flat bars from a foot on the
  trunk up to the blade.
- The right end plate shows past the roof, at the blade's far end.

**Front**

- The hood's three vents show as dark openings with raised front lips,
  in a line across the rear half of the hood.
- The splitter is a long flat carbon plate ahead of the bumper. On its
  left side a black upright end plate stands at the back of its side
  edge, just ahead of the front wheel; the right one shows at the far
  corner.
- Silver rods run from the splitter's front edge up to the bumper.
- The red shackle on its black bracket, right of the middle.

**Left side**

- 776 on the door, the black 5.0 badge on the fender ahead of it, the
  body coloured door handle, the black mirror.
- The door glass is down; the quarter glass is dark.
- The black side skirt, standing out a little from the body.
- A small red reflector low on the rear bumper's corner.
- The wheels are black with about ten thin spokes.

## 2024 04 06-7278: the left side from a little behind

The camera is level with the car, a little behind it; the car moves to
the left (nose left). The background is blurred from the pan; the body is
sharp, the wheels blurred.

**Left side**

- 776 on the door in black, heavy narrow digits with flat tops, the 7
  toward the nose. The digits stand about a third of the door's height
  (about 0.3 m), their tops just under the crease that runs along the
  door at the handle's height, and the number sits a little behind the
  door's middle.
- The black 5.0 badge on the front fender, ahead of the door and behind
  the wheel arch, in the fender's upper third.
- The body coloured door handle at the door's upper back, on the crease.
- The black mirror; the door glass down; the dark quarter glass behind
  it with a black frame.
- The black side skirt runs the whole way between the wheels, slim.
- The hood is dark charcoal carbon here in the sun; a vent's fins stand
  up above the hood's surface in its middle.
- The splitter's end plate is a black plate standing along the
  splitter's side edge, from the bumper's front back to just ahead of
  the front wheel.
- The wheels read dark grey rather than black in this light, with many
  thin spokes.

**Back** (seen at a slant from the left)

- The trunk lid is black, with a raised black lip across its back edge.
- The round fuel door on the left rear quarter, flush with a thin seam,
  between the rear wheel and the taillight, its middle level with the
  middle of the taillight.
- The three red taillight bars each side in a black surround; the panel
  between them dark with a small black pony.
- A black plate with white characters and a light frame, in the recess
  in the middle of the bumper.
- 776 small on the bumper, left of the plate, level with the plate's
  middle and close to it, about three quarters of the plate's height.
- A red reflector strip low on the bumper's corner.
- The black diffuser with upright fins, two exhaust tips each side, and
  the white reversing light right of the middle.

**Wing**

- The blade meets the Texas plates just under the panhandle; two silver
  bolts on the left plate.
- The left plate reads the right way round from outside, El Paso toward
  the nose; its southern tip hangs down to the trunk's height behind
  the car.
- The right upright shows clearly: a triangle standing on the trunk,
  a base along the trunk, a post up to the blade, and a brace from the
  base's front up to the blade behind the post.

## 2024 04 06-8998: the front and the left side from a little below

The camera is ahead of the car and to its left, a little below it; the
photo is tilted. Sharp. A grey Honda S2000 with "SUPER LAP BATTLE" across
its windshield follows behind: none of its parts belong to this car.

- The hood's three vents sit close to the windshield, in the rear part
  of the hood: a long one in the middle and a shorter one each side.
- The splitter's end plates: one upright at its right front corner as
  seen from here, one at the back of its left side edge just ahead of
  the front wheel. Silver rods from its front edge up to the bumper.
- The red shackle on its black bracket right of the middle.
- The windshield's yellow 3 sticker at its top corner on the car's left,
  the inspection sticker at its bottom corner on the car's left.
- 776 on the left door, the black 5.0 badge on the fender ahead of it,
  the black mirror.
- Both Texas end plates show, the right one seen from inside past the
  roof, the left one from outside at the blade's near end.
- The wheels are black with thin spokes.

## 2024 04 06-9131: the left side square on

The camera is level with the car's middle, a little above it, square to
its left side; the car moves to the left (nose left). The background is
blurred from the pan; the body is sharp, the wheels blurred. The best
photo for where things are along the car.

**Front half**

- The hood is dark charcoal carbon. Its edge runs along the top of the
  fender where the fender turns down, from the headlight back to the
  windshield; below it the fender is blue.
- The hood vent nearest the camera stands in the hood's middle, about a
  third of the way back from its front edge to two thirds: black louvre
  fins standing up above the hood's surface, with light showing between
  them.
- The black 5.0 badge on the front fender, about 0.65 m behind the front
  axle and about 0.25 m above it, ahead of the door's front edge.
- The headlight with the three LED bars and the amber strip; the white
  fog light strip low on the bumper's corner.
- The splitter reaches about 0.25 m ahead of the bumper, low. Its end
  plate is a black rectangle standing along its side edge, from the
  bumper's front back to just ahead of the front wheel, with a small
  bolt in it. A silver rod runs up from its front to the bumper. The red
  shackle shows at the bumper's front.
- The black mirror; the pillar beside the windshield is blue.

**Middle**

- 776 on the door, heavy narrow black digits, their tops just under the
  crease at the handle's height, ending a little ahead of the handle.
- The body coloured door handle on the crease at the door's upper back.
- The door glass is down; the quarter glass is dark with a black frame.
- The black side skirt the whole way between the wheels.
- The roof is black; a thin blue band of the roof's side shows above
  the side glass.

**Back half**

- The round fuel door on the left rear quarter, flush, with a thin dark
  seam; the quarter's crease runs across it.
- The trunk lid is black, a raised black lip along its back edge, the
  black reaching to the lid's edges and no further.
- The taillight wraps round the corner, red.
- The wing: the blade runs just under the Texas panhandles. The plates
  reach about a quarter metre behind the back of the car, and their
  southern tip hangs down to the trunk's height.
- Two silver bolts on each plate near the blade.
- Each upright is a triangle of flat black bars on a flat foot on the
  trunk: a base along the trunk, a post up to the blade, and a brace from
  the base's front up to the blade.
- The left plate reads the right way round from outside, El Paso toward
  the nose.

## 2024 04 06-9184: close on the left door's window, mirror and roof

The camera is close, level with the window, on the car's left; the nose
is to the left. Sharp.

- The roof is gloss black, reflecting the sky.
- Between the roof's black and the top of the side glass runs a blue
  band, the roof's side, about as tall as the mirror's housing.
- The side glass opening has a thin black frame.
- The door glass is down.
- The mirror is gloss black: a rounded housing on a black arm from a
  black triangular base at the front corner of the side glass.
- The windshield's yellow 3 sticker shows at its top corner on this side.
- The top of the door's 776 shows at the bottom: heavy black digits.
- The body coloured door handle at the right edge, on the door's crease.

## 2024 04 07-2354: the front and the left side from a little below

The camera is ahead of the car and to its left, a little below it; the
car climbs past. Sharp, the wheels a little blurred.

- In the sun the grille's pony is silver grey, not dark.
- The splitter: a small upright end plate at its right front corner as
  seen from here, and a black end plate at the back of its left side
  edge, just ahead of the front wheel; a silver rod from its front edge
  up to the bumper near the left corner.
- The red shackle on its black bracket right of the middle, above the
  lower intake.
- The windshield's yellow 3 sticker at its top corner on the car's left,
  the blue and white inspection sticker at its bottom corner on the
  car's left.
- The hood's vents in a line across its rear part.
- 776, the black 5.0 badge, the black mirror, the body coloured handle
  on the left side; the door glass down.
- A small red reflector on the rear bumper's corner.
- The wheels are black with about ten thin spokes.
- The left Texas plate shows at the top corner, seen from outside.

## 2024 04 07-2428: the front nearly head on

The camera is ahead of the car, a little to its left and above; the car
comes toward it, leaning into a turn. Sharp. The best photo for the
front.

**Hood**

- Satin black, covering the hood out to the tops of the fenders; its
  front edge sits just above the headlights, with a band of blue bumper
  between it and the grille.
- Three vents in a line across it close to the windshield: a long one
  in the middle, a shorter one each side. Each is a trapezoid opening,
  dark, with a raised lip along its front edge catching the light, and
  small rivets round its outline.

**Grille and bumper**

- The grille is six-sided, a black honeycomb, the pony in its middle,
  and a slanted black bar near each end.
- The headlights: projector, the three white LED bars on the inner side,
  amber along the outer lower edge.
- Each bumper corner has a black upright slot with a white strip light
  across it.
- Under the bumper bar a wide black honeycomb lower intake.
- The tow hook: a square black bracket with a bolt, on the bumper bar
  under the grille's right third (the car's right), a red D shackle
  hanging from it.

**Splitter**

- A flat black carbon plate about 0.2 m out ahead of the bumper, its
  front edge following the bumper's curve, reaching out past both
  bumper corners.
- Four thin silver rods from its front edge up to the bumper's lower
  edge: two inner ones a little either side of the tow hook's line, two
  outer ones under the headlights.
- A small black end plate stands upright at its right end.

**Top**

- The windshield's yellow 3 sticker at its top corner on the car's left,
  the blue and white inspection sticker at its bottom corner on the
  car's left.
- The pillars beside the windshield are blue, the roof black.
- Both mirrors are gloss black.
- The wing's blade and left upright show past the roof on the right.

## 2024 04 07-3661: the front and the right side, level

The camera is level with the car, ahead of it and to its right; the car
moves to the right (nose right). The background is blurred from the pan;
the body is sharp.

- The right Texas plate is seen from outside: mirrored, the panhandle
  up and El Paso pointing to the nose, as on the left plate (corrected;
  see 1073).
- The right upright: a black triangle from the trunk up to the blade.
- 776 on the right door, centred between the door's front edge and the
  handle, with the 6 toward the nose.
- The black 5.0 badge on the right fender ahead of the door.
- The door glass is down; the quarter glass is dark with a black frame.
- The splitter's right end plate stands along the back of its side edge,
  just ahead of the front wheel; its left end shows at the far corner.
  Silver rods from its front edge up to the bumper.
- The red shackle on its black bracket right of the middle.
- A red reflector strip low on the rear bumper's corner.
- The wheels are black with about ten thin spokes.

## 2024 04 07-3745: the front from above, head on

The camera is above the car and straight ahead of it; the photo is
turned about a third of a turn, so the car runs from bottom (nose) to
top right (tail). Sharp. The best photo for the hood.

**Hood**

- Dark carbon with the weave visible, two raised ridges running back
  from its front edge.
- The three vents in a line parallel to the foot of the windshield, in
  the rear part of the hood: the middle one a long narrow opening
  across the hood, one each side smaller and squarer. Each is a dark
  opening with mesh inside, framed by a raised flange with rivets along
  it.
- The hood's black runs out to the tops of the fenders on both sides.

**Front**

- The honeycomb grille, its slanted bars and dark pony; the honeycomb
  lower intake.
- The tow hook: a black square bracket with a round bolt head, on the
  bumper bar between the grille and the lower intake, on the car's right
  of the middle; the red D shackle hangs from it.
- The white strip lights in the black corner slots.
- The splitter from above: a wide flat carbon plate, its front edge
  following the bumper's curve, four thin silver rods up to the bumper.

**Also**

- The blue and white inspection sticker on the windshield at the car's
  left.
- The wing's blade and an upright show beyond the roof.

## The front: where each part is and how big

Every part added to the model on the front, with its place and size.
The body itself (the bumper, grille, headlights, fog lights, lower
intake, hood shape) is the downloaded Mustang's own and is not listed.

**How it was measured.** A grid laid over each photo, the photo used only
in the direction it shows square on, or through a render taken from the
photo's own camera:

- **Across the car**: 3745 turned level (26 degrees) so the grille runs
  straight across. Shares of H, the hood's half width at the vents' line
  (the middle to where the hood's black meets the fender). The camera
  stands a little to the car's left, so each left and right pair is
  averaged.
- **Along the car, square on**: 9131, square to the left side. Shares of
  L, the hood's length (its front edge above the headlights back to the
  foot of the windshield), and metres against the front wheel (0.72 m
  tall with its tyre, 521 pixels to the metre on the grid).
- **Along the car, seen from ahead**: 3745 looks along the hood, which
  squashes its length. A place there is written as seen in 3745, as a
  share of the way from the windshield's foot (0) to the hood's front
  edge (1) in the level picture. It is checked against a render taken
  from 3745's camera, not turned into metres by eye.

Each part says **measured** (read off a photo) or **judged** (not
measurable in these photos).

| Part | Place | Size | From | |
|---|---|---|---|---|
| Middle flat vent | Across: centred, 0.27 H either side. Along, as seen in 3745: back edge 0.10, front edge 0.50 of the way from the windshield's foot to the hood's front edge | Across 0.54 H at its back, narrowing to 0.85 of that at its front | 3745 level | measured |
| Side vents | Across: 0.35 H to 0.65 H out each side, a gap of 0.08 H to the middle one; their edges slant inward toward the front. Along: as the middle one. Each has a raised front lip standing up off the hood with louvre fins in it, the part seen from the side as fins (9131, 7278, 1073) | Across 0.30 H at the back, narrowing to 0.85 of that; the lip is the vent's front third | 3745 level, 1164 | measured |
| Scoops | None. The hood has three vents only (1164 shows all three and nothing else). The louvre fins seen from the side (9131, 7278, 1073) are the side vents' raised front lips | | 1164 | measured |
| Tow hook | Across: 0.29 H right of the middle. Up: on the bumper bar between the grille's bottom and the lower intake's top | Bracket 0.09 H across; shackle 0.10 H across, 0.12 H tall as seen | 3745 level | measured |
| Splitter | Reaches 0.22 m out ahead of the bumper's front, low. Its ends reach 1.25 H out from the middle, 0.17 H past the bumper's corners (1.08 H) | Front edge following the bumper's curve | 9131 (reach), 3745 level (ends) | measured |
| Splitter end plates | From 0.48 m to 0.72 m ahead of the front axle, on each side edge | 0.24 m long, 0.13 m tall | 9131 | measured |
| Splitter rods | Across: two at 0.19 H either side of the middle, two at 0.69 H. From the splitter's front edge up to the bumper's lower edge | Thin | 3745 level | measured |
| 5.0 badges | Middle 0.66 m behind the front axle, 0.26 m above it, on each front fender | 0.14 m across, 0.05 m tall | 9131 | measured |
| 3 sticker | Windshield, top corner on the car's left: its middle 0.78 H out from the middle, just under the windshield's top edge | 0.08 H across | 3745 level | measured |
| Inspection sticker | Windshield, bottom corner on the car's left: its middle 0.86 H out from the middle, just above the windshield's foot | 0.12 H across | 3745 level | measured |
| Hood's black | Out to the hood's seams with the fenders (0.83 m out at the windshield, 0.78 m at the headlights; the fender tops stay blue); from the windshield's foot to its front edge above the headlights and grille, down to the headlights at its front corners | | 3745 (the seams, cast onto the body) | measured |

## Which downloaded body

Two free 2018 Mustang GT bodies were measured against the reference
points (the same readings, lens and fit, windshield corners left out):

| Body | Photos whose camera solves (4 or more points agree) | Points agreeing |
|---|---|---|
| "Mustang_GT 2018" by Jesterz86 (the first used) | 5 of 11 | 36 of 93 |
| "2018 Ford Mustang GT (S550)" by supercarmodels (candidate B) | 11 of 11 | 52 of 90 |

Candidate B's size also matches Ford's: 2.07 m wide with mirrors (Ford
2.08), 1.37 m tall (Ford 1.38); the first came in 2.28 m wide. So the
model is built on candidate B (`candidate-b/`, CC BY, supercarmodels on
Sketchfab: https://sketchfab.com/3d-models/2018-ford-mustang-gt-s550-123bf7e107d64530a5b862aba4afb8b2).
Points that fail on both bodies by the same amount are photo misreads,
not shape, and are read again.

## Reference points

Sharp features on the car's body, the same spot on the model and on the
real car, read in every photo that shows them. Each photo's camera is
solved from them (OpenCV solvePnP); the model drawn from that camera over
the photo then measures every added part in pixels. Only the body's own
features: none of the parts added to the model.

The readings are in `refpoints.json` next to this file: for each photo,
each point's pixel (x, y) in the full size photo, 0,0 at the top left.
170 readings of 40 points across all 12 photos. 9184 is close on the
door and has 4 (a fit around the door only).

Our own parts (the 776s, the 5.0 badges, the tow hook) are not the
model's body, so they are kept apart in `checkpoints.json`, for checking
where the model's added parts sit once each photo's camera is solved:
15 readings in 9131, 7278 and 2428.

| Point | The spot, as read in a photo | Photos |
|---|---|---|
| pony | The middle of the pony in the grille | 1164, 7148, 7237, 8998, 2354, 2428, 3661, 3745 |
| top led bar front left / right | The front end of the top of the three LED bars | left: 1164, 7148, 7237, 7278, 8998, 9131, 2354, 2428, 3745; right: 1073, 1164, 7148, 8998, 2428, 3661, 3745 |
| amber tip left / right | The rear, outer tip of the headlight's amber strip | left: 7237, 7278, 9131; right: 1073, 3661 |
| grille top corner left / right | The grille opening's top corner on that side | left: 1164, 7148, 7237, 8998, 2354, 2428, 3745; right: 1164, 7148, 8998, 2428, 3661, 3745 |
| grille outer point left / right | The grille opening's outer point on that side | left: 7237, 8998, 2354, 2428, 3745; right: 1164, 7148, 2428, 3661, 3745 |
| mirror tip left / right | The mirror housing's outer end | left: 7237, 8998, 2354, 2428, 3745; right: 7237, 8998, 2354, 2428, 3661, 3745 |
| handle front / rear, left / right | The door handle's front and rear ends | left: 7148, 7237, 7278, 8998 (front), 9131, 2354; right: 1073, 1164, 3661 |
| quarter glass tip left / right | The rear tip of the small window behind the door, with its black frame | left: 7237, 7278, 9131; right: 1073, 1164, 3661 |
| fuel door middle left | The middle of the round fuel door | 7278, 9131 |
| windshield top corner left / right | Where the black band across the windshield's top meets the blue pillar | left: 1164, 7148, 7237, 8998, 9131, 9184, 2354, 2428, 3745; right: 1164, 2428, 3661, 3745 |
| windshield bottom corner left / right | The foot of the blue pillar beside the windshield, where it meets the cowl | left: 1164, 7148, 7237, 7278, 8998, 9131, 2354, 2428, 3745; right: 1073, 1164, 2428, 3661, 3745 |
| mirror housing rear top left / right | The top of the mirror housing's rear edge, seen from the side | left: 7278, 9184; right: 1073 |
| mirror arm foot left | Where the mirror's arm meets the door | 7278, 9184 |
| plate top / bottom, left / right | The rear plate's four corners | 1073, 7278 |
| reversing light outer end | The outer end of the white reversing light in the diffuser. Not used to fit: the model's light sits in the middle, the car's off it, so it is not the same spot | 1073, 7278 |
| taillight top inner left / right | The top, inner corner of the red taillight | left: 7278, 9131; right: 1073 |
| front / rear arch top, left / right | The top of the wheel arch opening | left: 7148, 7237, 7278, 8998, 9131, 2354; right: 1073, 1164, 3661 |
| front / rear hub, left / right | The middle of the wheel's hub | left: 7237, 7278, 8998, 9131, 2354; right: 1073, 1164, 3661 |

**How precise.** Every photo read on a 25 or 50 pixel grid, good to
about 5 to 10 pixels. The plate corners and the reversing light's outer
end (1073, 7278) cover the back.

**Not used to fit the camera**: the hubs and arch tops move with the
suspension and are for measuring the ride height; the model stands at
stock height.

**On the model** (step 3, done): each point was picked in square on,
orthographic pictures of the model (`views.py render`: left, right,
front, rear, top) and traced into it (`views.py cast`). The picks, as
view and pixel, are in `picks.json`; the 3D positions (metres, nose +x,
the car's left +y, up +z, ground z = 0) in `modelpoints.json`. 39 points.
The windshield's top corners are picked from the front: from the side a
ray passes over the pillar and hits the roof further in. Left and right
pairs come out mirrored to within about a centimetre.

## Where the photos disagree

- **The hood vents (settled by 1164).** The front photos show three
  vents; the side photos show louvre fins. 1164 shows the whole hood
  with three vents and nothing else, so the fins are the side vents'
  raised front lips. Two earlier readings here were wrong: first that
  the vents sat against the windshield, then that the hood had five
  (three vents and two scoops).
- **The wheels' colour.** Gloss black in 1073, dark grey in 7278 in
  stronger sun. Black, catching the light.
- **The hood's colour.** Black in the front photos, dark charcoal in the
  side photos in full sun. Black carbon, lit by the sun.

## Each part, and the photos that show it best

| Part | What it is | Best photos |
|---|---|---|
| Paint | Bright metallic blue (Velocity Blue) | all |
| Roof | Black, gloss; a blue band of the roof's side above the side glass; a black band across the top of the windshield | 9184, 1164, 2428 |
| Hood | Black carbon, faint weave, two raised ridges; out to its seams with the fenders (the fender tops blue), its front edge just above the headlights | 3745, 2428, 9131 |
| Hood vents | Three only (measured under "The front: where each part is and how big"): dark mesh vents from 0.10 to 0.50 of the way from the windshield's foot to the hood's front edge as seen in 3745 and 1164, the middle one 0.27 H either side, the side ones from 0.35 H to 0.65 H, narrowing toward the front; the side ones with a raised louvred front lip | 3745, 1164, 2428 |
| Grille | Six-sided black honeycomb, pony in the middle (blacked out, sRGB (33, 40, 57) in 2428 and 1164; silver grey in sun), a slanted bar near each end | 2428, 3745, 2354 |
| Headlights | Projector, three white LED bars on the inner side, amber along the outer lower edge | 2428, 1164 |
| Bumper corners | Black upright slots with a white strip light across | 2428, 3745 |
| Lower intake | Wide black honeycomb under the bumper bar | 2428, 3745 |
| Tow hook | Black square bracket with a bolt on the bumper bar between the grille and the lower intake, 0.29 H right of the middle; red D shackle hanging from it | 3745, 2428, 1164 |
| Splitter | Flat black carbon plate 0.22 m out ahead of the bumper, front edge following the bumper's curve, past the bumper's corners | 9131, 2428, 3745 |
| Splitter rods | Four thin silver rods from the splitter's front edge to the bumper: two at 0.19 H either side of the middle, two at 0.69 H | 3745, 2428 |
| Splitter end plates | A black plate 0.24 m long and 0.13 m tall standing on each side edge of the splitter, from 0.48 m to 0.72 m ahead of the front axle | 9131, 7278, 3661, 7237 |
| Door number | 776 in heavy narrow black digits with flat tops, about 0.3 m tall, on each door's lower middle under the crease | 9131, 7278, 1073 |
| 5.0 badge | Black, on each front fender ahead of the door, its middle 0.66 m behind the front axle and 0.26 m above it; 0.14 m across, 0.05 m tall | 9131, 7278, 1073 |
| Mirrors | Black carbon, gloss, on a black arm from a triangular base | 9184, 1164, 2428 |
| Side glass | Door glass down on both sides; the small quarter glass dark with a black frame | 9131, 1164 |
| Door handles | Body coloured, on the crease at each door's upper back | 9131, 7278 |
| Side skirts | Slim black blade the whole way between the wheels | 9131, 7278 |
| Wheels | Black, about ten thin spokes curving into a deep dish, open middle with the hub and five nuts; brake disc and black caliper behind | 1073, 7278, 3661 |
| Racetrack stickers | About five small white track outlines along the bottom of the dark glass at the back of the roof, on the right side only | 1073 |
| Fuel door | Round, flush, thin seam, on the left rear quarter between the rear wheel and the taillight, level with the taillight's middle | 9131, 7278 |
| Trunk lid | Black, a raised black lip across its back edge, the black reaching the lid's edges only | 9131, 7278, 1073 |
| Wing blade | Black, nearly the car's width, about level with the back of the roof, over the tail | 9131, 1164, 7278 |
| Uprights | Each a triangle of flat black bars on a flat foot on the trunk: a base along the trunk, a post up to the blade, a brace from the base's front up to the blade | 7278, 9131, 1073 |
| Texas end plates | Flat black, the panhandle up; the same shape on both sides, El Paso toward the nose (so the left reads the right way round from outside, the right reads mirrored); the blade meets them just under the panhandle; their southern tip hangs to the trunk's height, a quarter metre behind the car; two silver bolts on each near the blade | 9131, 1073, 7237, 3661 |
| Windshield stickers | Yellow square with a black 3 at the top corner on the left; small blue and white inspection sticker at the bottom corner on the left | 1164, 2428, 2354 |
| Taillights | Three red upright bars each side in a black surround, the outer one wrapping the corner | 1073, 7278 |
| Rear panel | Gloss black between the taillights, a small black pony and thin lines | 1073, 7278 |
| Plate | Black, white characters (TEXAS across the top between stars, ending in REAL, the rest unreadable), MUSTANG on the light frame, in the recess in the bumper's middle | 1073, 7278 |
| Rear 776 | Black, heavy narrow digits about three quarters of the plate's height, left of the plate, level with its middle, close to it | 7278 |
| Rear reflectors | A red strip low on each rear bumper corner | 1073, 7278, 3661 |
| Diffuser | Black with upright fins, two round steel exhaust tips each side, a white reversing light right of the middle | 1073, 7278 |
