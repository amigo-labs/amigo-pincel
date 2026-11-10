# Changelog

## [1.10.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.9.0...v1.10.0) (2026-11-10)


### Features

* **ui:** add a Button component with busy state and shortcut keycap ([aad8bce](https://github.com/amigo-labs/amigo-pincel/commit/aad8bcee088d94e633bc7982ad05c7b3da4906f0))
* **ui:** give buttons a pressed state, a danger variant and one on-state ([b1e9e4e](https://github.com/amigo-labs/amigo-pincel/commit/b1e9e4e376c25353851e2c3e4a9b510fa1d3ab2d))
* **ui:** one button language, themed tooltips, duotone icon set ([e64a198](https://github.com/amigo-labs/amigo-pincel/commit/e64a19847667f3ae764ab7ab46cd1885bc08045f))
* **ui:** redraw the icon set in duotone ([1e525e6](https://github.com/amigo-labs/amigo-pincel/commit/1e525e619c52542a7c56001c7de763a7f631c23d))
* **ui:** show themed tooltips with the shortcut as a keycap ([db9c59c](https://github.com/amigo-labs/amigo-pincel/commit/db9c59cbc4f1b3c8f0a33dc6069ee8e6157683e1))


### Bug Fixes

* **ui:** keep control names and nested titles under the tooltip layer ([e320368](https://github.com/amigo-labs/amigo-pincel/commit/e3203688f8f8e47134e056a1e09f133d9f86d272))
* **ui:** make the Slice and Move Selection Outline icons readable ([5d15c3d](https://github.com/amigo-labs/amigo-pincel/commit/5d15c3dde157156fd11f635eac93a326249038f6))

## [1.9.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.8.1...v1.9.0) (2026-11-10)


### Features

* **ui:** lay the start screen out as a weighted action grid ([140c864](https://github.com/amigo-labs/amigo-pincel/commit/140c864caeeddfa6cd9aed56433f950825457bed))
* **ui:** let the start screen layout follow the recent files ([31e3376](https://github.com/amigo-labs/amigo-pincel/commit/31e33762fb50295e4034f32e070bf39d41878309))
* **ui:** say in the New dialog what Create will make ([c489fff](https://github.com/amigo-labs/amigo-pincel/commit/c489fff1eb1d829b81ef99ed6c73c44ea75797b4))


### Bug Fixes

* re-sync agent skills (acceptance-criteria headings in any case) ([cbb40a5](https://github.com/amigo-labs/amigo-pincel/commit/cbb40a567d985d76ef2ee294ca381678ebdb4f37))
* re-sync agent skills (commit gate, spec-build completion) ([e6dc12e](https://github.com/amigo-labs/amigo-pincel/commit/e6dc12e8a19cba6950710572eee2911b8a03896b))
* **ui:** keep the New dialog's notes, Tab and size honest ([714987d](https://github.com/amigo-labs/amigo-pincel/commit/714987d5c47258b3c8ca24d7e78cc4c648c4a37e))

## [1.8.1](https://github.com/amigo-labs/amigo-pincel/compare/v1.8.0...v1.8.1) (2026-11-09)


### Bug Fixes

* **ui:** rename the tour store so Tour.svelte cannot shadow it ([6ec7d91](https://github.com/amigo-labs/amigo-pincel/commit/6ec7d9138b791f6e8563edcafb0b54453b8925bb))
* **ui:** rename the tour store so Tour.svelte cannot shadow it ([8d0bc91](https://github.com/amigo-labs/amigo-pincel/commit/8d0bc915ccbcb145ea095ec410f0d8a1d668cc82))

## [1.8.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.7.0...v1.8.0) (2026-11-09)


### Features

* finish the open feature specs (7 accepted) ([8411d6f](https://github.com/amigo-labs/amigo-pincel/commit/8411d6f1a1d8ee1371897027cefeb0bcbb123cfc))
* implement the open feature specs ([7084aed](https://github.com/amigo-labs/amigo-pincel/commit/7084aedb75028405170a7c849b46d7220c5f27ba))
* **open-files:** implement handling of OS-provided file paths before renderer is ready ([3bcd2b3](https://github.com/amigo-labs/amigo-pincel/commit/3bcd2b36a30034a81fc7d0036c945af169740166))
* **pincel-agent:** accept spiral_cw and spiral_ccw in draw.gradient (AC-10, AC-11, AC-12, AC-13) ([34703b0](https://github.com/amigo-labs/amigo-pincel/commit/34703b0a77308d53c3a9b70539d56adf70531629))
* **pincel-agent:** accept underline and strikethrough in draw.text (AC-20) ([2fbaa44](https://github.com/amigo-labs/amigo-pincel/commit/2fbaa445e64a32771e6b5da09e88a03a442b4817))
* **pincel-agent:** add content moves and the headless clipboard (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16, AC-17, AC-18, AC-20, AC-21, AC-22) ([d494997](https://github.com/amigo-labs/amigo-pincel/commit/d494997c9510d2672f0a98bec96273ddb2a49e35))
* **pincel-agent:** add export.svg and document the file tools (AC-01, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-26) ([4e6ec0d](https://github.com/amigo-labs/amigo-pincel/commit/4e6ec0d7ca26060d9e613fbd1ef01b6de49b8094))
* **pincel-agent:** add Image recolor, smudge and per-mode effect defaults (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16, AC-17) ([374e2b5](https://github.com/amigo-labs/amigo-pincel/commit/374e2b517d6e7136b55b1c98533358a78b4f5fc3))
* **pincel-agent:** add image.auto_crop and supersampling to image.scale (AC-17, AC-18, AC-19, AC-20) ([ea9a7e7](https://github.com/amigo-labs/amigo-pincel/commit/ea9a7e7265897c1f58506cbfbda7ae80abcf0b1a))
* **pincel-agent:** add layer duplicate, move, merge and flatten tools (AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-28, AC-29) ([46efed9](https://github.com/amigo-labs/amigo-pincel/commit/46efed9bfffdd80c83e5d46533f4faaf091f9324))
* **pincel-agent:** add rotate_zoom to the effect.apply catalog (AC-10) ([2230ca2](https://github.com/amigo-labs/amigo-pincel/commit/2230ca274aef322d6444ccc26de78532b6c37464))
* **pincel-agent:** add select.* and transforms for both modes (AC-01, AC-02, AC-03, AC-04, AC-05, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16, AC-17, AC-18, AC-19, AC-20, AC-21, AC-22, AC-23, AC-24, AC-25, AC-26, AC-27, AC-28, AC-29, AC-30, AC-31, AC-32, AC-33, AC-34, AC-35, AC-36, AC-37, AC-38) ([a6c7bf6](https://github.com/amigo-labs/amigo-pincel/commit/a6c7bf62fe5120d0a40bf50375c69eeef6408f84))
* **pincel-agent:** add select.fill for Image documents (AC-10 – AC-13) ([a45b6d1](https://github.com/amigo-labs/amigo-pincel/commit/a45b6d11b7d96ba10e379d6d8444f663e918856a))
* **pincel-agent:** add slice, tileset, tilemap and grid tools (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16, AC-17, AC-18, AC-19, AC-20, AC-21, AC-22, AC-23, AC-24, AC-25, AC-26, AC-27, AC-28, AC-29, AC-30, AC-31, AC-32, AC-33, AC-34, AC-35) ([c928b0d](https://github.com/amigo-labs/amigo-pincel/commit/c928b0def7fc0222bb9d0d282fc1eb1ea494492c))
* **pincel-agent:** add tag.set, palette entry and palette file tools (AC-01, AC-02, AC-14, AC-15, AC-16, AC-17, AC-18, AC-19, AC-20, AC-21, AC-22, AC-24, AC-25, AC-26, AC-27, AC-30) ([9caf0c0](https://github.com/amigo-labs/amigo-pincel/commit/9caf0c042d94cce1b5c224b02b208d84d00a5c5d))
* **pincel-agent:** add the draw.clone MCP tool (AC-19, AC-20) ([b244b1c](https://github.com/amigo-labs/amigo-pincel/commit/b244b1c62f576be336539969e80eb4ba116ca76c))
* **pincel-agent:** anti_alias on draw.stroke, select.ellipse and select.polygon (AC-14 – AC-16) ([7103cb3](https://github.com/amigo-labs/amigo-pincel/commit/7103cb3e31fab5f39d70963bba2ade5698224dde))
* **pincel-agent:** blend on the painting tools and the guide text (AC-14, AC-16, AC-17) ([cc629fa](https://github.com/amigo-labs/amigo-pincel/commit/cc629fa1a9a012a18a2756f29841965209ea2cc2))
* **pincel-agent:** doc.open reads TGA, DDS and ICO (AC-15, AC-16) ([a0deb9e](https://github.com/amigo-labs/amigo-pincel/commit/a0deb9e61d56e85115f3b792fd3aa6fdcf136433))
* **pincel-agent:** draw.shape draws triangle, diamond, star, block_arrow, heart and callout (image-shapes-curves-shapes AC-16 – AC-18) ([177d706](https://github.com/amigo-labs/amigo-pincel/commit/177d70633b07d2f1c5f92962932b4b26935dc254))
* **pincel-agent:** draw.shape lines take control points, caps and dash-dot patterns (image-shapes-curves AC-18 – AC-20) ([7c80f54](https://github.com/amigo-labs/amigo-pincel/commit/7c80f5402940dc2bccae2b99724934a6dee4bbb5))
* **pincel-agent:** draw.stroke, shape, text and gradient on Pixel documents (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16, AC-17, AC-18, AC-19, AC-20, AC-21, AC-22, AC-23, AC-24) ([9e6bf45](https://github.com/amigo-labs/amigo-pincel/commit/9e6bf45181c14f87a1e49b63d0cb1638f8c1fc6c))
* **pincel-agent:** export Pixel animations as APNG and GIF (AC-02, AC-11, AC-12, AC-13, AC-27) ([e4e8614](https://github.com/amigo-labs/amigo-pincel/commit/e4e8614d562037adc3880e843e19f950a2ec4f31))
* **pincel-agent:** name Image MCP steps after the editor action they mirror (AC-15) ([49603b3](https://github.com/amigo-labs/amigo-pincel/commit/49603b383e8501d7ff20ec6da2ca5743b0d76795))
* **pincel-agent:** name Pixel MCP steps after the editor action they mirror (AC-15) ([9321c33](https://github.com/amigo-labs/amigo-pincel/commit/9321c330447e025117776a5ed5c4bbcc424901bd))
* **pincel-agent:** open PSD files and PNG as Pixel documents (AC-21, AC-22, AC-23, AC-24, AC-25, AC-29) ([48d5a5a](https://github.com/amigo-labs/amigo-pincel/commit/48d5a5a62df4284311fde799fc27263dbaf18828))
* **pincel-agent:** save Image documents in flat formats and export BMP / TIFF (AC-14, AC-15, AC-16, AC-17, AC-18, AC-19, AC-20, AC-28) ([c2d5e54](https://github.com/amigo-labs/amigo-pincel/commit/c2d5e54745f893ef75e5798f62d5db6e5431efa6))
* **pincel-core:** a revision counter that moves with the history's step list (AC-08) ([8d7f2b0](https://github.com/amigo-labs/amigo-pincel/commit/8d7f2b0e0f8d166c47bea3acfdb2e567dd2e2037))
* **pincel-core:** add RotSprite and nearest rotation by any angle as RotateCel (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06) ([33fbd43](https://github.com/amigo-labs/amigo-pincel/commit/33fbd435dd1de6534edd4854ecc30aecb5193d68))
* **pincel-core:** history kinds and multi-step jump on the bus (AC-03, AC-05, AC-12, AC-13, AC-15) ([b7350ae](https://github.com/amigo-labs/amigo-pincel/commit/b7350aea862aa28d09622e9842b4ce016c51c769))
* **pincel-core:** history kinds for several layer or tag properties at once (AC-15) ([2a3eaa5](https://github.com/amigo-labs/amigo-pincel/commit/2a3eaa5c399cbf40df3a34ad5c93e08d982e3cd1))
* **pincel-effects:** add the affine resampler and the rotate_zoom effect (AC-04, AC-05, AC-06) ([d9c949d](https://github.com/amigo-labs/amigo-pincel/commit/d9c949d03ba4bd45e07157bbe6c31cd880b0cc53))
* **pincel-image-core:** add a live text kind and the text-layout query (AC-20, AC-22) ([8b7a815](https://github.com/amigo-labs/amigo-pincel/commit/8b7a815b5d1f6d399c4eda22361dd8064ab4c7d8))
* **pincel-image-core:** add AutoCrop command and supersampling spec name (AC-12, AC-13, AC-14, AC-15) ([2211175](https://github.com/amigo-labs/amigo-pincel/commit/221117552278b91cb29618a4f814d0dc5f2247df))
* **pincel-image-core:** add clockwise and counter-clockwise spiral gradient kinds (AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09) ([8a9910f](https://github.com/amigo-labs/amigo-pincel/commit/8a9910fec3ae69a3d5e2e3e338358ff2aee47314))
* **pincel-image-core:** add Fill Selection command (AC-02 – AC-09) ([2c9f6d3](https://github.com/amigo-labs/amigo-pincel/commit/2c9f6d362940d21ec17fc50b5c2108ab7f1c8eea))
* **pincel-image-core:** add live shapes, lines and gradients (AC-01..AC-16) ([ba790bd](https://github.com/amigo-labs/amigo-pincel/commit/ba790bd9622cad42cdb32411cf58fee16b95167a))
* **pincel-image-core:** add Supersampling area-average resampling (AC-10, AC-11) ([0dc524f](https://github.com/amigo-labs/amigo-pincel/commit/0dc524f5f365a0b32464dc003c517f9f5037fbe6))
* **pincel-image-core:** add the Clone Stamp tool and clone_stroke command (AC-05, AC-06, AC-08, AC-09, AC-10, AC-11, AC-12) ([38dfde1](https://github.com/amigo-labs/amigo-pincel/commit/38dfde1764da5f0a69bbc4dfadc533f7cafc3667))
* **pincel-image-core:** add underline, strikethrough and a text layout to the Text tool (AC-10, AC-22) ([4639962](https://github.com/amigo-labs/amigo-pincel/commit/463996268ceaea3543640a41ab4d3332448ed1e9))
* **pincel-image-core:** anti-aliased hard round tip and ellipse / lasso selections (AC-02 – AC-04, AC-06, AC-07) ([5e065d5](https://github.com/amigo-labs/amigo-pincel/commit/5e065d5b2bdc72a2fbd3ddfaa45956e5c43e6608))
* **pincel-image-core:** drop a live handle drag without a redo step (image-live-objects-ui AC-11) ([1b51b06](https://github.com/amigo-labs/amigo-pincel/commit/1b51b06de4a9e9d1e7b123cfbbd945976bb73802))
* **pincel-image-core:** history kinds instead of labels, and a multi-step jump (AC-03, AC-05, AC-12, AC-15) ([edb1a70](https://github.com/amigo-labs/amigo-pincel/commit/edb1a7010c3fa9649dbde60fb20997cd88996df7))
* **pincel-image-core:** Line / Curve rasterizer with caps, arrow heads and dash-dot patterns (image-shapes-curves AC-01, AC-05 – AC-11, AC-14, AC-16, AC-21) ([173d87c](https://github.com/amigo-labs/amigo-pincel/commit/173d87cc7fe82407350547abd3d348481aca4dab))
* **pincel-image-core:** live lines take control points and caps; draw_shape lines curve (image-shapes-curves AC-15, AC-16, AC-18) ([5344a8e](https://github.com/amigo-labs/amigo-pincel/commit/5344a8e28e8e90157253d589092853b00ed8e27d))
* **pincel-image-core:** open TGA, DDS and ICO; write TGA and ICO (AC-01 – AC-05, AC-11, AC-12) ([0bdfe18](https://github.com/amigo-labs/amigo-pincel/commit/0bdfe189f76f7a25896c69b0dda89f8f9c12aa9a))
* **pincel-image-core:** Overwrite blending for Brush, Bucket, Gradient, Shapes and Text (AC-08 – AC-13) ([1f12885](https://github.com/amigo-labs/amigo-pincel/commit/1f12885e62f34393c364fac04b35446b6482ce71))
* **pincel-image-core:** the six new shapes as live box shapes with star options (image-shapes-curves-shapes AC-19, AC-20) ([cc6616a](https://github.com/amigo-labs/amigo-pincel/commit/cc6616a8a571849e21d689445662f24178ac4bce))
* **pincel-image-core:** Triangle, Diamond, Star, Block Arrow, Heart and Callout shapes (image-shapes-curves-shapes AC-02 – AC-10, AC-13 – AC-15) ([92af05a](https://github.com/amigo-labs/amigo-pincel/commit/92af05a4d8fa3e03a00fedac135fd9a9297d5ae0))
* **pincel-image-wasm:** add document snapshots for the job worker (AC-01, AC-02) ([65fa727](https://github.com/amigo-labs/amigo-pincel/commit/65fa7279e04a6eaad530fb9e42be6833740a923d))
* **pincel-image-wasm:** add no undo step for an effect that changes no pixel (AC-03, AC-05) ([f11480f](https://github.com/amigo-labs/amigo-pincel/commit/f11480f2591f85d1dd79e8404231d7a00e0875af))
* **pincel-image-wasm:** compute and commit effect regions apart (AC-07) ([27960e4](https://github.com/amigo-labs/amigo-pincel/commit/27960e42cd51cfda1f657a96d0119e465862e858))
* **pincel-image-wasm:** expose document_dpi and the auto_crop command (AC-01, AC-13) ([feff31c](https://github.com/amigo-labs/amigo-pincel/commit/feff31cd9064b047c4f70741ac8beae1f1001eef))
* **pincel-image-wasm:** expose history kinds and a multi-step jump (AC-03, AC-04, AC-05, AC-15) ([c665b01](https://github.com/amigo-labs/amigo-pincel/commit/c665b0159b2c41c39f268b1bd4aa103e8f6051ab))
* **pincel-image-wasm:** expose live objects and commit them on layer change (AC-08, AC-13) ([8e61d9b](https://github.com/amigo-labs/amigo-pincel/commit/8e61d9b8d48084f485e92b83f8155b432dd47f53))
* **pincel-image-wasm:** expose the text-layout query and live text commands (AC-22) ([a341755](https://github.com/amigo-labs/amigo-pincel/commit/a3417550f645cc9466c479cc8d15891e2461e9d1))
* **pincel-image-wasm:** shape_outline gives the core's polygon of a box shape for the drag preview (image-shapes-curves-shapes AC-12) ([39612bd](https://github.com/amigo-labs/amigo-pincel/commit/39612bd40adbbd54b56afcaa838f8bbbaa656de4))
* **pincel-image-wasm:** TGA hint on open, export_tga / export_ico, image_to_png ([62caec0](https://github.com/amigo-labs/amigo-pincel/commit/62caec07f86ae5b39eeebe5b0676217580f5029d))
* **pincel-ocr:** port Tesseract's LSTM line recognizer (AC-01, AC-02, AC-05, AC-06, AC-07, AC-09, AC-10) ([b0025d2](https://github.com/amigo-labs/amigo-pincel/commit/b0025d24ce407a8b9dfa79d65edbd1ed54fe06e9))
* **pincel-ocr:** port Tesseract's single-line page layout ([cb210cb](https://github.com/amigo-labs/amigo-pincel/commit/cb210cb3ef3be76e1e35f5cfc0eb16f10a8c4ee2))
* **pincel-ocr:** port Tesseract's single-line page layout (AC-03, AC-04, AC-08, AC-11) ([ed397e2](https://github.com/amigo-labs/amigo-pincel/commit/ed397e2ebc4a4d0397a500149feda3051331b46c))
* **pincel-wasm:** add setSelectionMask to restore a saved selection (AC-25) ([73bb09e](https://github.com/amigo-labs/amigo-pincel/commit/73bb09eda5591affd10265fb914feaf7e52bfb30))
* **pincel-wasm:** add strokeToBatch for one frame of samples (AC-01, AC-02) ([c4de827](https://github.com/amigo-labs/amigo-pincel/commit/c4de827ba5a790375ac846bc59b2ecf0a80a14c4))
* **pincel-wasm:** expose history kinds and a multi-step jump (AC-03, AC-04, AC-05, AC-13, AC-15) ([f3be84e](https://github.com/amigo-labs/amigo-pincel/commit/f3be84e76882bba8c652baac56721c987aed9d1a))
* **pincel-wasm:** expose rotateLayer, previewRotate and the RotSprite cap (AC-05, AC-06, AC-08, AC-09, AC-10, AC-13) ([e5dd838](https://github.com/amigo-labs/amigo-pincel/commit/e5dd8386c1d8404d382e420570ca94753cb90f25))
* **pincel-wasm:** expose the history revision (AC-08) ([68c7161](https://github.com/amigo-labs/amigo-pincel/commit/68c71610e1e410e2068bc501ccf19df75330b972))
* save dialog revision, live text, spiral gradients, Rotate / Zoom, Fill Selection, tablet gestures ([ecf5864](https://github.com/amigo-labs/amigo-pincel/commit/ecf5864f8fbc723c9a83c3c49bfa1ff55f5e9a33))
* **tauri:** Help ▸ Tour in the native menu (first-run-tutorial AC-12) ([5d1756b](https://github.com/amigo-labs/amigo-pincel/commit/5d1756bc6029cbfc7a987e01682ed540fd1c1026))
* **ui:** add Edit &gt; Fill Selection to Image mode (AC-01, AC-02, AC-04 – AC-06, AC-08, AC-14) ([1bf117d](https://github.com/amigo-labs/amigo-pincel/commit/1bf117d73a88ea032ef5eb3335ddf684e9f909e3))
* **ui:** add latest-only scheduling for superseded jobs (AC-06, AC-08) ([6b2cbdf](https://github.com/amigo-labs/amigo-pincel/commit/6b2cbdfa4779eb7f81ccf565c5fe306bbfd5975e))
* **ui:** add Layer &gt; Rotate / Zoom to Image mode (AC-01, AC-02, AC-03, AC-05, AC-06, AC-08, AC-09) ([ea45578](https://github.com/amigo-labs/amigo-pincel/commit/ea45578d54b27fe80c501cbdd621f501788be969))
* **ui:** add Layer &gt; Rotate... dialog with live RotSprite preview (AC-07, AC-08, AC-09, AC-10, AC-11, AC-13) ([05b4669](https://github.com/amigo-labs/amigo-pincel/commit/05b466972f23dd7c13733e3e152ae7dc8acd20d0))
* **ui:** add the Clone Stamp to Image mode (AC-01 … AC-18) ([173c109](https://github.com/amigo-labs/amigo-pincel/commit/173c1091b1a59251acc082673520cc1c6ac022e1))
* **ui:** apply toolbar, color and shape changes to the live object (AC-06, AC-07, AC-08) ([955e7ba](https://github.com/amigo-labs/amigo-pincel/commit/955e7ba669ab3bc2f00b997c7d37d4f85ab45118))
* **ui:** commit the live object before tab switch, copy, effects, export and save (AC-09, AC-14, AC-15) ([d8bcb1f](https://github.com/amigo-labs/amigo-pincel/commit/d8bcb1f96da309848618122fef802cc82ade5f32))
* **ui:** Ctrl+N opens New in the desktop app and an installed window (AC-14, AC-20) ([3cf0cc4](https://github.com/amigo-labs/amigo-pincel/commit/3cf0cc4c92fbbf8f644cd40a4724280bf92c071b))
* **ui:** dim the top bar except Effects and Adjustments while an effect is open (AC-03) ([16a6a28](https://github.com/amigo-labs/amigo-pincel/commit/16a6a282e9183bae2d0af284c862f2baa30d8a6d))
* **ui:** effect panel as a movable modal dialog over the canvas area (AC-01, AC-02, AC-04, AC-12, AC-13, AC-14, AC-15, AC-18, AC-19, AC-20, AC-21) ([5b32777](https://github.com/amigo-labs/amigo-pincel/commit/5b32777f3085e93f6ead02b639340d831ce222cd))
* **ui:** English and German names for history steps and the History panel (AC-04, AC-16) ([658856a](https://github.com/amigo-labs/amigo-pincel/commit/658856a4ae404117503d8bc80dd9da211cef5221))
* **ui:** File ▸ Print… in Image mode with fit-to-page and actual size (AC-01 – AC-11, AC-13) ([8cd0db7](https://github.com/amigo-labs/amigo-pincel/commit/8cd0db7d7fa7abc9a4342e863b47eb7ca9e0f35d))
* **ui:** first-run tour with start-screen banner, editor hints and Help ▸ Tour (first-run-tutorial AC-01 – AC-11, AC-17, AC-18) ([13befed](https://github.com/amigo-labs/amigo-pincel/commit/13befed3a6519898c30cb27bef233f19ce68447e))
* **ui:** hex, HSV, recent-list and palette helpers for the color picker (AC-05, AC-06, AC-07, AC-11, AC-23) ([e8d3bf2](https://github.com/amigo-labs/amigo-pincel/commit/e8d3bf285d0905b3bef0b24588441431a0785f2f))
* **ui:** History panel component, step names and document origin (AC-02, AC-03, AC-04, AC-07, AC-10, AC-17, AC-18) ([609be13](https://github.com/amigo-labs/amigo-pincel/commit/609be13f1aa431c7ceab5d82727ea38d49fe6f34))
* **ui:** History panel in Image mode (AC-01, AC-03, AC-05, AC-08, AC-09, AC-11, AC-12, AC-13, AC-14) ([8089cfd](https://github.com/amigo-labs/amigo-pincel/commit/8089cfd5736849f502305c82e4ee66862a0d50d2))
* **ui:** History panel in Pixel mode (AC-01, AC-03, AC-05, AC-08, AC-09, AC-11, AC-12, AC-13, AC-14) ([70288e0](https://github.com/amigo-labs/amigo-pincel/commit/70288e05979d273b8e6487c06fac02f4f9ec2f12))
* **ui:** Image &gt; Auto Crop with a nothing-to-crop notice (AC-12 – AC-16) ([cf6ee4c](https://github.com/amigo-labs/amigo-pincel/commit/cf6ee4c5a024f2a6f9030521683e0bc27b314a6b))
* **ui:** Image mode paints and samples with the color's alpha (AC-17, AC-22) ([024535f](https://github.com/amigo-labs/amigo-pincel/commit/024535f62a4d4abcff479260840f77b27e9ab526))
* **ui:** Image rulers, pixel grid and units in View (AC-01 – AC-08, AC-10 – AC-21, AC-23 – AC-25) ([3add075](https://github.com/amigo-labs/amigo-pincel/commit/3add075ccffa4e17c5672544f1e96df5bc5c8164))
* **ui:** keep Image shapes, lines and gradients live with handles (AC-01, AC-10, AC-12, AC-16, AC-17) ([ff06083](https://github.com/amigo-labs/amigo-pincel/commit/ff060839f05bbe23651ee4f629a3e7d9f23b6d6c))
* **ui:** keep recent colors and the last colors with alpha across reloads (AC-15, AC-30) ([303f6ad](https://github.com/amigo-labs/amigo-pincel/commit/303f6adca7189b8308a71620edeae0e7fbe240a2))
* **ui:** Line / Curve tool with control points, caps, dash-dot patterns and the curve's move grip (image-shapes-curves AC-01 – AC-04, AC-12, AC-13, AC-15, AC-17, AC-22) ([b501708](https://github.com/amigo-labs/amigo-pincel/commit/b501708e0ace8cf17c843359209bb24c5fdbca51))
* **ui:** mirror the live text commands in the Image adapter (AC-10, AC-20, AC-22) ([253a0d8](https://github.com/amigo-labs/amigo-pincel/commit/253a0d8ea181ac9fd344c2b7b85e99b33009f0dc))
* **ui:** mirror the live-object commands in the Image adapter (AC-01..AC-17) ([5a429ad](https://github.com/amigo-labs/amigo-pincel/commit/5a429adb1a497b467c50452e9dbfedfa9f17dbf3))
* **ui:** names for the Image-mode history steps (AC-04, AC-16) ([f3b0438](https://github.com/amigo-labs/amigo-pincel/commit/f3b043844933e83a8f488f7e6d9abcd25233d1d2))
* **ui:** names for the layer and tag properties history steps (AC-04) ([4ac65e1](https://github.com/amigo-labs/amigo-pincel/commit/4ac65e155271d81f2a94704fb2dfad3a301a4965))
* **ui:** offer Spiral (clockwise) and Spiral (counter-clockwise) in the Image Gradient options (AC-01, AC-03, AC-08, AC-14) ([27d38bd](https://github.com/amigo-labs/amigo-pincel/commit/27d38bd06d6143cdf6f922124c01386cc2f8b780))
* **ui:** one Antialiasing option for Image mode's edge tools (AC-01 – AC-07) ([d5ce7e7](https://github.com/amigo-labs/amigo-pincel/commit/d5ce7e7cf7b4c91e3a201665ee76a3a1a101b2d2))
* **ui:** one Blending option (Normal / Overwrite) for Image mode's painting tools (AC-08 – AC-10) ([ca1c731](https://github.com/amigo-labs/amigo-pincel/commit/ca1c7310471ec7fec76033e4464b9f3bcfb576c9))
* **ui:** one dialog per Image-mode Save (AC-07, AC-10, AC-15, AC-16, AC-17) ([dd810e9](https://github.com/amigo-labs/amigo-pincel/commit/dd810e99edab8ed71f11fc788e9ca68e014b278d))
* **ui:** Open a sample on the start screen with a sprite and an image sample (first-run-tutorial AC-13 – AC-16) ([9c12eda](https://github.com/amigo-labs/amigo-pincel/commit/9c12eda3db31f439c30930764d4246709c7f7bfc))
* **ui:** open, drop and save TGA, DDS and ICO in Image mode (AC-06 – AC-10, AC-13, AC-14) ([5da9cf1](https://github.com/amigo-labs/amigo-pincel/commit/5da9cf10ce60eef0f721f1440eda792e1adcf508))
* **ui:** own color picker popover on both swatches, with alpha in both modes (AC-01, AC-02, AC-03, AC-04, AC-08, AC-09, AC-10, AC-11, AC-18, AC-19, AC-20, AC-21, AC-23) ([7c034ba](https://github.com/amigo-labs/amigo-pincel/commit/7c034bad44259dd667edcf0bdf1494703424308e))
* **ui:** percentage, print-size and Supersampling input in Scale Image and Resize Canvas (AC-01 – AC-09, AC-21, AC-22) ([8aa9cf9](https://github.com/amigo-labs/amigo-pincel/commit/8aa9cf997c203d107b9c20ae8893bf925e19d7f0))
* **ui:** pixel monogram as app icon ([0e0aff3](https://github.com/amigo-labs/amigo-pincel/commit/0e0aff31005c419c6584af3b75c9f1cfd484d0c0))
* **ui:** recent colors and default palette in Image mode's Colors panel (AC-25, AC-26, AC-27, AC-28, AC-29) ([9d020b4](https://github.com/amigo-labs/amigo-pincel/commit/9d020b49e8cfa2b9f043e65cdae72f11a110a627))
* **ui:** recompute the Image Magic Wand click as its settings change (AC-33, AC-34) ([9a845a1](https://github.com/amigo-labs/amigo-pincel/commit/9a845a1cd55ff9c623297881e4cae3c0d0bea5f1))
* **ui:** recompute the Pixel Magic Wand click on a settings commit (AC-25, AC-26) ([48feb87](https://github.com/amigo-labs/amigo-pincel/commit/48feb87dfa7a274ceed143fac1a74d5c4ad0e05c))
* **ui:** record the colors a tool paints with as recent colors per mode (AC-12, AC-13, AC-14, AC-24) ([f3fe953](https://github.com/amigo-labs/amigo-pincel/commit/f3fe9531f99b9e1d99a572d4624f7d8ececc9346))
* **ui:** replace the text entry with a live text in Image mode (AC-01, AC-02, AC-03, AC-09, AC-11, AC-15, AC-16, AC-21) ([e6d60c1](https://github.com/amigo-labs/amigo-pincel/commit/e6d60c1061ceb2cd7b90a7eb1bfb497ebf3ca388))
* **ui:** right button with the Image Zoom tool zooms out (AC-35) ([4a1bdbc](https://github.com/amigo-labs/amigo-pincel/commit/4a1bdbc8f2d9098a7f88ad37107cc3ef88cafed6))
* **ui:** tell browsers without wasm SIMD they are too old (AC-07) ([cda89c8](https://github.com/amigo-labs/amigo-pincel/commit/cda89c8057140cfdecc51edcb8b90e062e00d7d0))
* **ui:** Triangle, Diamond, Star, Block Arrow, Heart and Callout in the Shapes group (image-shapes-curves-shapes AC-01, AC-11, AC-12, AC-19, AC-20) ([bb1715e](https://github.com/amigo-labs/amigo-pincel/commit/bb1715e5039f7cb06860bcd61a4eb2ac563cab68))
* **ui:** two- / three-finger taps undo / redo and the pen eraser end erases (AC-01 – AC-14) ([5aacd2e](https://github.com/amigo-labs/amigo-pincel/commit/5aacd2e901ec804b59543118495694c2435290b4))
* **ui:** use the Pi pixel monogram as app and web icon ([057dafe](https://github.com/amigo-labs/amigo-pincel/commit/057dafe5748d5c1aaf1bcdb2ae454822383f969f))
* **website:** compare pixel-art and image editors in two honest tables (AC-01..AC-03, AC-05..AC-11) ([a10e031](https://github.com/amigo-labs/amigo-pincel/commit/a10e0311b752ab4429100001d6c4daa6f215320c))


### Bug Fixes

* **ci:** release profile and workflow test match the merged release setup ([c651dad](https://github.com/amigo-labs/amigo-pincel/commit/c651dadbcb222f95afc4e155b4b3827bad7e363a))
* **pincel-agent:** doc.open's description and the guide name TGA, DDS and ICO (image-formats-more AC-15) ([df373c9](https://github.com/amigo-labs/amigo-pincel/commit/df373c9d9407a8d070239f1d5a6811d9c63864dd))
* **pincel-agent:** list every drawing argument, default and selection rule in the guide (AC-22) ([022f3eb](https://github.com/amigo-labs/amigo-pincel/commit/022f3eb828ed795e0512006a5bdf212e4bf2af4e))
* **pincel-agent:** name the accepted forms when an argument is missing or unlisted (AC-02, AC-15) ([ae76c84](https://github.com/amigo-labs/amigo-pincel/commit/ae76c84c1d307bbbc123698145000bcc415b65e4))
* **pincel-agent:** name the accepted range when an argument has the wrong type (AC-09) ([c8ab4a5](https://github.com/amigo-labs/amigo-pincel/commit/c8ab4a50ed0dafe8924f5afc4176299fb682a1a1))
* **pincel-agent:** name the group when export.selection copies one in an Image document (AC-14) ([a8be419](https://github.com/amigo-labs/amigo-pincel/commit/a8be41937e1806cb2412c544266cc407c29b78da))
* **pincel-agent:** name the mode in every per-mode drawing range error (AC-24) ([bbe24f6](https://github.com/amigo-labs/amigo-pincel/commit/bbe24f62318b0c41f7b07042090fe23e5db80933))
* **pincel-agent:** record no undo step for an Image drawing call that changes nothing (AC-13, AC-21) ([96c8110](https://github.com/amigo-labs/amigo-pincel/commit/96c81100a212da74b6d656bf3694a9569cf6723b))
* **pincel-agent:** refuse a missing frame on Image clipboard tools (AC-20) ([6226b22](https://github.com/amigo-labs/amigo-pincel/commit/6226b22122186c7d22712693bdde43c7f969970c))
* **pincel-agent:** refuse a zero-offset layer.translate on a locked layer (AC-08) ([e70a302](https://github.com/amigo-labs/amigo-pincel/commit/e70a302099b5da8ee3f69a8feab76141db2b301a))
* **pincel-agent:** refuse importing an image doc.open refuses (AC-13) ([ae620ab](https://github.com/amigo-labs/amigo-pincel/commit/ae620ab6745fccf0a633fd251365d64ce7c59c4e))
* **pincel-agent:** refuse underline and strikethrough on Pixel draw.text (AC-18) ([15bd629](https://github.com/amigo-labs/amigo-pincel/commit/15bd62992f908af46e730793eaaf6ac41b5d1eb7))
* **pincel-agent:** shorten the quoted value of an unknown argument (image-edit-extras AC-11, AC-12) ([0e7b489](https://github.com/amigo-labs/amigo-pincel/commit/0e7b489aebcbc869a1be971f9c5d7ed7880b27f6))
* **pincel-agent:** suggest &lt;stem&gt;.aseprite when saving a PNG opened as Pixel (AC-23) ([279c827](https://github.com/amigo-labs/amigo-pincel/commit/279c82741db1cc98cf17a79b64ecb5f6070567e9))
* **pincel-agent:** suggest a .ora path first when doc.save gets an unwritable path (AC-19) ([ec1bf3c](https://github.com/amigo-labs/amigo-pincel/commit/ec1bf3cc4b582d80d85929bf7f5edd1693e6615e))
* **pincel-agent:** suggest a region inside the tile when tile.read_pixels reads outside it (mcp-parity-pixel-assets AC-16) ([e134e90](https://github.com/amigo-labs/amigo-pincel/commit/e134e900c3255d4e65b05b61d57b93d1ac9dd6ee))
* **pincel-agent:** write nothing when the core refuses an export.selection cut (AC-21) ([84f67f5](https://github.com/amigo-labs/amigo-pincel/commit/84f67f5e905929ef76cd2b0d73b6d00e513d6571))
* **pincel-core:** add no undo step for a fill, shape, text or gradient that writes no pixel ([0c8697c](https://github.com/amigo-labs/amigo-pincel/commit/0c8697c89a8ad4002615ee1edac04e922aa535e7))
* **pincel-core:** make the GIF median cut independent of hash order ([87e5185](https://github.com/amigo-labs/amigo-pincel/commit/87e5185d4712ac7df6d3e79cbd79a80e22accc9f))
* **pincel-image-core:** a stroke that reaches no pixel is no undo step (AC-24) ([f73c023](https://github.com/amigo-labs/amigo-pincel/commit/f73c0236ddc9042b78e024ef8f93756235a77ef0))
* **pincel-image-core:** count live objects in the undo byte cap (AC-03, AC-05) ([267b04d](https://github.com/amigo-labs/amigo-pincel/commit/267b04d93450ead8adf0ec12266e74411d3455e4))
* **pincel-image-core:** dashed outlines of symmetric box shapes are mirror symmetric (image-shapes-curves-shapes AC-10) ([a9ecbff](https://github.com/amigo-labs/amigo-pincel/commit/a9ecbff4b665cfd4e9ab7557b535adbeb959b420))
* **pincel-image-core:** paint the pixel a sub-pixel dab lies in ([e961ab8](https://github.com/amigo-labs/amigo-pincel/commit/e961ab8a9e71ae90be07309a07cd37e4b6b67792))
* **pincel-image-wasm:** keep one step per Apply for every effect but Rotate / Zoom (AC-05) ([1942dba](https://github.com/amigo-labs/amigo-pincel/commit/1942dba4925a000c79a02feaab8d6402c36adcd3))
* **pincel-ocr:** bound outline memory and refuse images beyond int16 ([838ea71](https://github.com/amigo-labs/amigo-pincel/commit/838ea71748e068b24b92ef438da60f73c9afecbe))
* **pincel-ocr:** satisfy clippy 1.99 (as_chunks, late init) ([590c920](https://github.com/amigo-labs/amigo-pincel/commit/590c92032e3e56fa86a9fbed3054fabeee949c28))
* **release:** optimize the wasm with the pinned binaryen, not Ubuntu's ([bf32cf6](https://github.com/amigo-labs/amigo-pincel/commit/bf32cf6e03119244285e60776dcc980a725f42b1))
* **release:** optimize the wasm with the pinned binaryen, not Ubuntu's ([b2d3801](https://github.com/amigo-labs/amigo-pincel/commit/b2d38011ecd7fffc78d32889e5f3bc31d2128a57))
* **ui:** a growing sidebar panel keeps a floor instead of spilling over the next panel ([cb08b21](https://github.com/amigo-labs/amigo-pincel/commit/cb08b2129d7d623edf20a7d235834785ba98666a))
* **ui:** a History jump resets the live text's caret and selection as Undo does (AC-11, AC-05) ([60edd2f](https://github.com/amigo-labs/amigo-pincel/commit/60edd2f1c22fd53c6227bc7c332a9e58b86fefc8))
* **ui:** align menu rows and calm the dialogs ([bbe79f6](https://github.com/amigo-labs/amigo-pincel/commit/bbe79f6e4296ca1ee98235a966fe141089ca9457))
* **ui:** close an empty Image text box on every commit trigger ([150f712](https://github.com/amigo-labs/amigo-pincel/commit/150f712a5b1b7e5ed5395fb4208fdcea099c19f2))
* **ui:** count live Image shapes, lines and gradients as painted ([75ba5df](https://github.com/amigo-labs/amigo-pincel/commit/75ba5df3fa35e0b320e7b21705e383a87ccd6868))
* **ui:** effect panel shows label and value on one row ([7ecef08](https://github.com/amigo-labs/amigo-pincel/commit/7ecef0871bf8b89b61cd112129c678080227c6e5))
* **ui:** Enter in an effect's number field applies the typed value clamped ([e15f850](https://github.com/amigo-labs/amigo-pincel/commit/e15f850ea07e1afa542b8d1160efd83270bcdd89))
* **ui:** forward the swatch alpha to live objects after the color-picker merge (AC-07) ([c09f413](https://github.com/amigo-labs/amigo-pincel/commit/c09f4132358ee25b70d1d7a6d8f9d2bd21da11b9))
* **ui:** group options-bar controls with dividers ([05d1e78](https://github.com/amigo-labs/amigo-pincel/commit/05d1e78f0436bd72c224779660b5e678e06025b8))
* **ui:** hide the Image pointer position when a pressed pointer leaves the canvas area (AC-24) ([bb1c248](https://github.com/amigo-labs/amigo-pincel/commit/bb1c24881c6df34858f294b0ff68766c8225d768))
* **ui:** hide the image pointer position when the first touch contact lifts ([96dcc02](https://github.com/amigo-labs/amigo-pincel/commit/96dcc02807099947c0b88403f8c6395b7ce4e73e))
* **ui:** hold a touch press on a live object until no pinch follows ([d082ab7](https://github.com/amigo-labs/amigo-pincel/commit/d082ab7fca558f0d19cb67bd15b7f64ab8e1750b))
* **ui:** keep an Image view toggle made before the stored view state loads ([27b5ab6](https://github.com/amigo-labs/amigo-pincel/commit/27b5ab6aa682a466f82c6d2394329ec3d7ab5d8e))
* **ui:** keep the focus in a live text a touch tap opened ([c0eaa7a](https://github.com/amigo-labs/amigo-pincel/commit/c0eaa7abaffb8b2b6626152e515b550d13a6bbc0))
* **ui:** keep the Image pointer position and ruler markers on the current view ([829dfa5](https://github.com/amigo-labs/amigo-pincel/commit/829dfa5d4a9307a71cba9322a2dc02310828bb73))
* **ui:** keep the Image Text tool's entry open after the click ([bd56e51](https://github.com/amigo-labs/amigo-pincel/commit/bd56e51bb84b246637e94fbc072d8e93df31ee64))
* **ui:** let a pen drag the picker's hue and alpha sliders (AC-08, AC-09) ([bbb14fd](https://github.com/amigo-labs/amigo-pincel/commit/bbb14fd92f255144b4791a8565d01479651074e1))
* **ui:** let the Image job worker instantiate the page's compiled core ([d5ef45c](https://github.com/amigo-labs/amigo-pincel/commit/d5ef45c077a77a41c06e7e5cac86a8dec56ecc30))
* **ui:** meet contrast and forced-colors floors from review ([e40a6cc](https://github.com/amigo-labs/amigo-pincel/commit/e40a6ccc5bbc26613ee5ff9f8125ce59d9e048e6))
* **ui:** meet contrast and forced-colors floors from review ([3450c69](https://github.com/amigo-labs/amigo-pincel/commit/3450c69b235b5a6c53c174656ad3100c0c683224))
* **ui:** meet contrast and forced-colors floors from review ([6887061](https://github.com/amigo-labs/amigo-pincel/commit/6887061675946cd2f259ced1f3bceeb843391d6d))
* **ui:** meet contrast and forced-colors floors from review ([153953c](https://github.com/amigo-labs/amigo-pincel/commit/153953c69c368a23dbefce54e5399245f80a6b71))
* **ui:** name the Resize Canvas anchor buttons in the UI language ([6c9f7dc](https://github.com/amigo-labs/amigo-pincel/commit/6c9f7dc7e43b9910915a81173d86efd80bbb19a2))
* **ui:** one selected and disabled look across rails and panels ([c25da2d](https://github.com/amigo-labs/amigo-pincel/commit/c25da2dc451a583e9e3d8a4c1a329b5cbd6f5a40))
* **ui:** re-measure the Image pixel grid when the device pixel ratio changes ([17550dc](https://github.com/amigo-labs/amigo-pincel/commit/17550dcc8fc88ac0157fa0913d34f847ca98fa44))
* **ui:** rebalance the start screen card ([5b9cdd9](https://github.com/amigo-labs/amigo-pincel/commit/5b9cdd9a683df906354a368b5675656d8d0a5d56))
* **ui:** Save of an ICO grown past 256 px no longer says ICO cannot be written back (image-formats-more AC-08) ([b581ce5](https://github.com/amigo-labs/amigo-pincel/commit/b581ce5c57d75dfa132e31d827e11dfc796c58c0))
* **ui:** store the last colors once per picker interaction, not on every move ([cae9ed2](https://github.com/amigo-labs/amigo-pincel/commit/cae9ed2636afb8838d715c06e3dcfe458e0cfd95))
* **ui:** style the About and file-association dialog buttons ([9dcdb58](https://github.com/amigo-labs/amigo-pincel/commit/9dcdb587306331903a2eb0bf6953b093ed2bd324))
* **ui:** theme form controls and raise contrast tokens to WCAG AA ([fe9693e](https://github.com/amigo-labs/amigo-pincel/commit/fe9693eeac7f71aaf383af20ee23f15ceaabfe61))
* **ui:** UI polish pass — contrast, themed controls, consistent states ([697200a](https://github.com/amigo-labs/amigo-pincel/commit/697200a328f99db35e21363bb76834f3f3e03693))


### Performance Improvements

* **build:** fat LTO and one codegen unit for release builds (AC-01) ([e5dd57c](https://github.com/amigo-labs/amigo-pincel/commit/e5dd57c2854db03cd2c882e0054214ac345a947d))
* **pincel-core:** expand and contract selections in O(w*h) ([c78385a](https://github.com/amigo-labs/amigo-pincel/commit/c78385a975ca7f5c6765788c7756f6ebac38b161))
* **pincel-image-core:** keep only changed tiles for undo (AC-01..AC-05) ([370202a](https://github.com/amigo-labs/amigo-pincel/commit/370202aa28d6c08c7e0257b28cfa5bcc953bd785))
* **ui:** batch Image brush samples per animation frame (AC-01, AC-02, AC-03) ([2601ddc](https://github.com/amigo-labs/amigo-pincel/commit/2601ddc23b37023ab8281e5b2fabce706781777f))
* **ui:** encode Image saves, exports and autosaves in a job worker (AC-01, AC-02, AC-03, AC-09, AC-10) ([2354177](https://github.com/amigo-labs/amigo-pincel/commit/235417715bfa656f306f2769abb4192cd1cd5b56))
* **ui:** feed Pixel pencil samples to the core once per frame (AC-01, AC-02) ([7362567](https://github.com/amigo-labs/amigo-pincel/commit/73625671a5f21abf4627987893e270415d8041ba))
* **ui:** run Image effect previews and applies in the job worker (AC-06, AC-07, AC-08, AC-09) ([7154e32](https://github.com/amigo-labs/amigo-pincel/commit/7154e328ad9a53836f491542b62d66210924094b))
* **wasm:** build both wasm cores with 128-bit SIMD (AC-02) ([ee944b2](https://github.com/amigo-labs/amigo-pincel/commit/ee944b2032ecc564450e5e40d0d8b030d93fefb7))

## [1.7.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.6.0...v1.7.0) (2026-11-05)


### Features

* paint.net effects, docked effect panel, document tabs and unsaved-changes prompt ([aba129c](https://github.com/amigo-labs/amigo-pincel/commit/aba129c4c074ebf972ed9d1f8e39d954144872cf))
* **pincel-agent:** colour parameter kind and the outline effect (AC-12) ([bbfd2c2](https://github.com/amigo-labs/amigo-pincel/commit/bbfd2c2e86026b6488a92db115d839f15b33569e))
* **pincel-agent:** expose Drop Shadow, Stylize Outline, Quantize and Reduce Noise's strength to agents ([68ea4f1](https://github.com/amigo-labs/amigo-pincel/commit/68ea4f19b24503a0844eda659e46c935b7071e39))
* **pincel-agent:** expose the paint.net adjustments to agents (AC-10) ([1043cc9](https://github.com/amigo-labs/amigo-pincel/commit/1043cc96b511a01cccfc34abc0992cc542a47759))
* **pincel-agent:** expose the paint.net Artistic and Photo effects to agents ([ede3e97](https://github.com/amigo-labs/amigo-pincel/commit/ede3e9753c2b0b762394a8e84d3fe54378922a2d))
* **pincel-agent:** expose the paint.net blurs to agents (AC-13) ([a3b1bbc](https://github.com/amigo-labs/amigo-pincel/commit/a3b1bbc6916220274e90f4d72d0ffe68bcb4f9cc))
* **pincel-agent:** expose the paint.net distortions to agents ([2daf47b](https://github.com/amigo-labs/amigo-pincel/commit/2daf47bfed1ec723a60f5754faef6a5c02807f21))
* **pincel-agent:** expose the paint.net render effects to agents ([72ee703](https://github.com/amigo-labs/amigo-pincel/commit/72ee703b61d36807b1eb8b262581c7b9513ef893))
* **pincel-core:** add the DrawDitherGradient command (dithered-gradient AC-01, AC-02, AC-03, AC-04, AC-06, AC-07, AC-08, AC-14, AC-15) ([719b1f4](https://github.com/amigo-labs/amigo-pincel/commit/719b1f42fc93b022e5e185c80acc99df23f482b4))
* **pincel-core:** add the Shading ink to PencilStroke (shading-ink AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-11) ([e9e93e7](https://github.com/amigo-labs/amigo-pincel/commit/e9e93e7386a076a0f97736e2c7456aad5bfb975c))
* **pincel-core:** compose indexed and grayscale sprites (AC-01 … AC-12) ([03233c8](https://github.com/amigo-labs/amigo-pincel/commit/03233c80c3c26b0d3ffa2e692caa5b512c7887ea))
* **pincel-core:** compose linked cels (AC-01 … AC-07) ([a0b4463](https://github.com/amigo-labs/amigo-pincel/commit/a0b446369289138b23697195a40d66f7f86dddbf))
* **pincel-core:** fold group opacity and blend mode in compose (AC-08 … AC-20) ([6767bae](https://github.com/amigo-labs/amigo-pincel/commit/6767bae95b2989af18591fe072ef7037a08a73c4))
* **pincel-core:** integer-scaled PNG frame and sheet export (AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09) ([9e7933c](https://github.com/amigo-labs/amigo-pincel/commit/9e7933c1dc0889413cc21f65aa9b3a46809e636a))
* **pincel-core:** mirror, wrap and cel growth in PencilStroke (AC-01, AC-02, AC-03, AC-05, AC-06, AC-07, AC-15, AC-16, AC-17, AC-19, AC-21) ([09eab06](https://github.com/amigo-labs/amigo-pincel/commit/09eab06a938c3ebd693245c0d4f06b54133cd5c9))
* **pincel-core:** read and write Lospec .hex, JASC .pal and PNG palette strips (palette-formats AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-10, AC-11, AC-12, AC-13, AC-15) ([18cdce3](https://github.com/amigo-labs/amigo-pincel/commit/18cdce36bba2d540ba31199e5b9ab8b84de6f72d))
* **pincel-core:** undoable SetGrid command for the sprite grid (AC-08, AC-09, AC-10, AC-13) ([d0ab686](https://github.com/amigo-labs/amigo-pincel/commit/d0ab686f962b839a0abd407ebcdf5ba4c88fe61e))
* **pincel-effects:** add Drop Shadow, Stylize Outline, Quantize and paint.net Reduce Noise (AC-03 – AC-10) ([ac5d299](https://github.com/amigo-labs/amigo-pincel/commit/ac5d29976b94c2782d251310e939f87b54ab4972))
* **pincel-effects:** add the Outline Object effect with a colour wire slot (AC-02, AC-03, AC-04, AC-05, AC-06, AC-12, AC-13) ([c08b586](https://github.com/amigo-labs/amigo-pincel/commit/c08b5869bac358a4006c8076316ea95f889abe7c))
* **pincel-effects:** add the paint.net adjustments (AC-03 – AC-09, AC-11) ([868d4d6](https://github.com/amigo-labs/amigo-pincel/commit/868d4d6026cbfedda0df2cb2985bef6016a9e79b))
* **pincel-effects:** add the paint.net Artistic effects (AC-05, AC-06, AC-07) ([3dafe15](https://github.com/amigo-labs/amigo-pincel/commit/3dafe154081139156750b87f2955693b47e5c4d3))
* **pincel-effects:** add the paint.net blurs (AC-03 – AC-11, AC-13, AC-14) ([6bb9973](https://github.com/amigo-labs/amigo-pincel/commit/6bb99735bed1494379de6edd45a3e97c875d2457))
* **pincel-effects:** add the paint.net distortions (AC-03 – AC-12) ([cdfb309](https://github.com/amigo-labs/amigo-pincel/commit/cdfb3094d2e042b7577e4b9af5dbae4afea66021))
* **pincel-effects:** add the paint.net Photo effects (AC-03, AC-04, AC-08 – AC-11) ([32ad03b](https://github.com/amigo-labs/amigo-pincel/commit/32ad03bc99efc60f338fd419526fc09887a04a65))
* **pincel-effects:** add the paint.net render effects (AC-03 – AC-08) ([1122d87](https://github.com/amigo-labs/amigo-pincel/commit/1122d87e59f82c90c9ba2ec572ba6aa9ff627301))
* **pincel-effects:** outline effect with a colour wire slot (AC-02, AC-03, AC-04, AC-05, AC-06, AC-12, AC-13) ([c18400e](https://github.com/amigo-labs/amigo-pincel/commit/c18400efbe7c3b64fce85f547562004990604c7c))
* **pincel-image-core:** commands report the composite rect they change (AC-01) ([c2b0443](https://github.com/amigo-labs/amigo-pincel/commit/c2b0443422707f59cdda578183ec250b83846384))
* **pincel-image-core:** composite layer groups in isolation (AC-07, AC-08, AC-09, AC-10) ([e60c6d4](https://github.com/amigo-labs/amigo-pincel/commit/e60c6d4ab3e59e5a21e1728ef6ff84f76d089d9e))
* **pincel-image-core:** group commands in the Image command format (AC-01, AC-02, AC-03, AC-14) ([e63da69](https://github.com/amigo-labs/amigo-pincel/commit/e63da69eb19fdca4fdb8a91e52cb881aece1dec8))
* **pincel-image-core:** layer groups in the Image-mode layer tree (AC-01, AC-02, AC-04, AC-05, AC-20) ([ee30c68](https://github.com/amigo-labs/amigo-pincel/commit/ee30c68acb3c815ba263a434b3aaae130a1c5c51))
* **pincel-image-core:** merge rules for layer groups (AC-06, AC-13, AC-14, AC-15) ([2deb168](https://github.com/amigo-labs/amigo-pincel/commit/2deb1686b608a845e8d36bf4374990e147d3fa04))
* **pincel-image-core:** paste above a group and keep group buffers out of canvas ops (AC-19) ([bdb2420](https://github.com/amigo-labs/amigo-pincel/commit/bdb2420d2c203fed5454ff53cdc1fc16ba01d94e))
* **pincel-image-core:** read 8-bit RGB PSD layers, groups and composite (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-10, AC-14) ([adb9e41](https://github.com/amigo-labs/amigo-pincel/commit/adb9e415054cd86e1b93679758a6145ff14319d1))
* **pincel-image-core:** read and write layer groups as nested OpenRaster stacks (AC-01, AC-02, AC-03, AC-04, AC-05, AC-07) ([e6169bd](https://github.com/amigo-labs/amigo-pincel/commit/e6169bd22839dd42af8738c2d3bd0b9d55110997))
* **pincel-image-core:** refuse pixel edits on groups and move group content (AC-11, AC-12, AC-18) ([8e35db5](https://github.com/amigo-labs/amigo-pincel/commit/8e35db57eb1b19537c573f644479af159062b8df))
* **pincel-image-wasm:** expose the open-document count for tests (AC-16) ([570508e](https://github.com/amigo-labs/amigo-pincel/commit/570508eb7a27e7139bcb4081bda607d3c361ab29))
* **pincel-image-wasm:** layer tree in the document info and group thumbnails (AC-11, AC-12, AC-17) ([c395705](https://github.com/amigo-labs/amigo-pincel/commit/c395705ddc233f92d3fa45c0e7f4309e8850469e))
* **pincel-image-wasm:** open_psd returns the handle and the import notices (AC-06, AC-09) ([3b3059e](https://github.com/amigo-labs/amigo-pincel/commit/3b3059e7e2b8655896d0653d8025771db62da4a4))
* **pincel-image-wasm:** run effects on the selection as their target (AC-10) ([7427dc3](https://github.com/amigo-labs/amigo-pincel/commit/7427dc342611cf7c1621e7ec903a7f02aa52e70f))
* **pincel-wasm:** add beginShadingStroke for the Pencil's Shading ink (shading-ink AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-11, AC-13) ([7fc8a37](https://github.com/amigo-labs/amigo-pincel/commit/7fc8a370f064927b68e4e2f0529ffd87bc45fd44))
* **pincel-wasm:** add composeFrame(frame, zoom, onion) for onion-free composites (animation-preview AC-09) ([fbd01bb](https://github.com/amigo-labs/amigo-pincel/commit/fbd01bb7a095d318d47a2e15542977fc50dbb846))
* **pincel-wasm:** add importPalette / exportPalette for every palette format (palette-formats AC-06, AC-07, AC-08, AC-09, AC-14, AC-15) ([cdca03c](https://github.com/amigo-labs/amigo-pincel/commit/cdca03cf06fd5a9e428a61bc4a68360b11e19705))
* **pincel-wasm:** anchor Pixelate to the canvas grid for trimmed cels (AC-06, AC-13, AC-14) ([e4365fc](https://github.com/amigo-labs/amigo-pincel/commit/e4365fc22fde92c6d2bff1e3bad8e7b4d3feebde))
* **pincel-wasm:** count live documents for tests (AC-16) ([f177eb8](https://github.com/amigo-labs/amigo-pincel/commit/f177eb8fe287eb38991e8094dcb772b4bb078ad7))
* **pincel-wasm:** expose applyGradient for the dithered Gradient tool (dithered-gradient AC-08, AC-12, AC-13) ([a3bba13](https://github.com/amigo-labs/amigo-pincel/commit/a3bba131eb21afe1827ea7287c08ad00ab27bda0))
* **pincel-wasm:** grid accessors and undoable setGrid (AC-08, AC-09, AC-10, AC-13) ([2a3fdbc](https://github.com/amigo-labs/amigo-pincel/commit/2a3fdbc816595ecfb13fc7c6590da2e4e7d24872))
* **pincel-wasm:** optional scale on exportPng and exportAtlas (AC-03, AC-04, AC-06, AC-07, AC-08, AC-09) ([799207f](https://github.com/amigo-labs/amigo-pincel/commit/799207f95035e07f1168e8d84b08acab264f6a41))
* **pincel-wasm:** run effects on the selection as their target (AC-10, AC-11, AC-13) ([9c5b9b9](https://github.com/amigo-labs/amigo-pincel/commit/9c5b9b98a6f862cce5cc1f752f5975e86d9719b3))
* **pincel-wasm:** setStrokeMirror and setStrokeWrap for the stroke gesture (AC-01, AC-02, AC-03, AC-15) ([37164b1](https://github.com/amigo-labs/amigo-pincel/commit/37164b128a102ac1e2f062b0ac3aa1bc3f818704))
* **tauri:** add File &gt; Close with CmdOrCtrl+W to the native menu (AC-20) ([b2fd942](https://github.com/amigo-labs/amigo-pincel/commit/b2fd942f3127ada68fc0043d2e2772f35183a9f6))
* **tauri:** associate .psd files with Pincel as a viewer (AC-13) ([7261633](https://github.com/amigo-labs/amigo-pincel/commit/72616332274f84bfaeeca35a971820db31cb4fe1))
* **tauri:** Help ▸ Keyboard Shortcuts in the native macOS menu (AC-03) ([39426a5](https://github.com/amigo-labs/amigo-pincel/commit/39426a59b5a31b4422633d7236c408a0cbfbc830))
* **tauri:** let the UI ask before the window closes (AC-09) ([5a80305](https://github.com/amigo-labs/amigo-pincel/commit/5a80305b84c5913f284127d1182812d5f1254451))
* **tauri:** move the native Export Sprite Sheet accelerator to Cmd+Alt+E (power-user-shortcuts-coverage AC-19) ([85c6702](https://github.com/amigo-labs/amigo-pincel/commit/85c67022cf739448887938806c7ea1a322ceb178))
* **tauri:** register the clipboard-manager plugin with read-image only (AC-27, AC-28, AC-33) ([9691418](https://github.com/amigo-labs/amigo-pincel/commit/9691418b53d78e882772845fc3a55010aba7c223))
* **tauri:** set_menu_accelerators rebuilds the macOS menu with rebound chords (AC-14) ([c890ece](https://github.com/amigo-labs/amigo-pincel/commit/c890ece753d61d3077dce0a6449cc9dc2749c935))
* **ui:** add a Ctrl+K command palette over every menu command and tool (power-user-shortcuts-palette AC-01 – AC-17) ([b445733](https://github.com/amigo-labs/amigo-pincel/commit/b445733b369f7c43aefb5673cd64bf236880c0e9))
* **ui:** add a Preview panel that plays the animation while you draw (animation-preview AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16) ([14477e2](https://github.com/amigo-labs/amigo-pincel/commit/14477e280e96a14714d334997770d7929ef9dd5c))
* **ui:** add the dithered Gradient tool to Pixel mode's Fill group (dithered-gradient AC-09, AC-10, AC-11, AC-12, AC-13) ([4ed0364](https://github.com/amigo-labs/amigo-pincel/commit/4ed03644633ad89c776aeebf19feaffe8e4a0d72))
* **ui:** add tool-rail flyout model helpers (AC-02, AC-04, AC-05, AC-08, AC-11) ([e507fe3](https://github.com/amigo-labs/amigo-pincel/commit/e507fe352794464bf65bf7c2370ce2defdd45eba))
* **ui:** ask about unsaved changes before the desktop app closes (AC-01, AC-08, AC-09, AC-10, AC-11) ([924d878](https://github.com/amigo-labs/amigo-pincel/commit/924d878114ae7f2768770f92eabcc3c955c98efe))
* **ui:** ask Save / Don't Save / Cancel before replacing unsaved work (AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-12, AC-13) ([0a2ac68](https://github.com/amigo-labs/amigo-pincel/commit/0a2ac6873c56dd63543c261410e27c371629cbea))
* **ui:** canvas only zooms and pans while an effect is open (AC-03) ([a009345](https://github.com/amigo-labs/amigo-pincel/commit/a009345c10852414ab99ba97de2a37d89e594e48))
* **ui:** default shortcuts for layers, frames, flips, rotation, opacity, the palette and Toggle UI (power-user-shortcuts-coverage AC-01 – AC-19) ([85f381e](https://github.com/amigo-labs/amigo-pincel/commit/85f381e3bf8994718200b29827780a5476186139))
* **ui:** dock the effect panel in the sidebar, drawer or bottom sheet (AC-01, AC-02, AC-04 – AC-06, AC-12 – AC-15) ([c2536b6](https://github.com/amigo-labs/amigo-pincel/commit/c2536b68cd98353fb2bc01c96082e2d79b1a6966))
* **ui:** drop images onto an open editor to add them as layers or open them (file-access AC-20, AC-21, AC-22, AC-23, AC-24, AC-25, AC-26, AC-27, AC-28) ([b75c1b7](https://github.com/amigo-labs/amigo-pincel/commit/b75c1b799b28d4fb8d3bd42f58b8649f51b1d280))
* **ui:** Drop Shadow, Stylize Outline, Quantize and paint.net Reduce Noise in the Effects submenus (AC-01, AC-02, AC-12) ([6560b2e](https://github.com/amigo-labs/amigo-pincel/commit/6560b2e26b2f6126ee0bb283f94eb70a8f0018be))
* **ui:** Effects &gt; Object &gt; Outline Object with a colour parameter in the effect panel (AC-01, AC-06, AC-08, AC-09, AC-11) ([4650965](https://github.com/amigo-labs/amigo-pincel/commit/4650965cb899539c222dde1b3a50cb86ab8ada57))
* **ui:** Effects ▸ Outline… with a colour parameter in both modes (AC-01, AC-08, AC-09, AC-11) ([964845b](https://github.com/amigo-labs/amigo-pincel/commit/964845bf61203765ae3d90a018ef55c23ff304b7))
* **ui:** export and import the keymap as JSON (AC-07 – AC-11, AC-15 – AC-17) ([fef54f3](https://github.com/amigo-labs/amigo-pincel/commit/fef54f3e0da0837ac13b859a52b166766229f7e0))
* **ui:** failed effects keep the panel, prompts and document switches end it (AC-16, AC-17) ([a7bcb71](https://github.com/amigo-labs/amigo-pincel/commit/a7bcb717d7799435c1bda25f2962cf64452d54e7))
* **ui:** Grid Settings dialog for the Pixel-mode grid (AC-07, AC-08, AC-09, AC-10, AC-13, AC-15, AC-17) ([b9bf15c](https://github.com/amigo-labs/amigo-pincel/commit/b9bf15c2111a2076098e6c7e1c5d8776e1b89bb6))
* **ui:** import any palette file and export .gpl, .hex, .pal or PNG strips (palette-formats AC-01, AC-02, AC-03, AC-05, AC-06, AC-09, AC-10, AC-11, AC-12) ([4670230](https://github.com/amigo-labs/amigo-pincel/commit/467023059b53ca111acbf7b386d0f1d3534ac18d))
* **ui:** keep the native macOS menu in step with the keymap (AC-14) ([c0a929b](https://github.com/amigo-labs/amigo-pincel/commit/c0a929b964ec2a3c12bfc860e842224a4c16ad8a))
* **ui:** keyboard shortcuts overview dialog (AC-01, AC-02, AC-04 – AC-18) ([44ced13](https://github.com/amigo-labs/amigo-pincel/commit/44ced131aa1f1f356a903b7e1642de847f885449))
* **ui:** layer groups in Image mode's layers panel and Layer menu (AC-01, AC-02, AC-03, AC-05, AC-07, AC-11, AC-12, AC-14, AC-16, AC-17, AC-20) ([62340eb](https://github.com/amigo-labs/amigo-pincel/commit/62340ebe1f1668ad09f6c2d533739c087568b2b3))
* **ui:** mix both modes in one tab bar (AC-01 – AC-05, AC-07 – AC-11, AC-15 – AC-17, AC-19, AC-21 – AC-25) ([35ab4fd](https://github.com/amigo-labs/amigo-pincel/commit/35ab4fd00da9caada19929bff11dd998152f10ac))
* **ui:** offer a clipboard image on the desktop start screen (AC-27 – AC-32) ([593daba](https://github.com/amigo-labs/amigo-pincel/commit/593daba33b1cef16dc5628ac4d4439c745ab27b8))
* **ui:** open a variant flyout from tool-rail group buttons (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14) ([6ffa0bd](https://github.com/amigo-labs/amigo-pincel/commit/6ffa0bd5700727d5249a43b36c1d07414f81e4ce))
* **ui:** open PSD files in Image mode (AC-01, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-14) ([ed992f8](https://github.com/amigo-labs/amigo-pincel/commit/ed992f857e43e8c6851e29e5d67325cc72d61e7e))
* **ui:** open several documents as tabs in each editor (AC-01 – AC-20) ([70cb8eb](https://github.com/amigo-labs/amigo-pincel/commit/70cb8eb7752eac047f0e9b7b6e47d60c4c4ca31b))
* **ui:** open the shortcut overview from the native Help menu (AC-03) ([31c5a7d](https://github.com/amigo-labs/amigo-pincel/commit/31c5a7d325e02a8c487fcadd1756fd1fc061120a))
* **ui:** paint.net adjustments in both modes' Adjustments menu (AC-01, AC-02, AC-10, AC-12) ([d4ea8a9](https://github.com/amigo-labs/amigo-pincel/commit/d4ea8a9c0605c880dcad17eb326662f6391aad04))
* **ui:** paint.net Artistic and Photo effects in Image mode's submenus (AC-01, AC-02, AC-13) ([e5c8fdf](https://github.com/amigo-labs/amigo-pincel/commit/e5c8fdfb7787358db70dbdb77d1ab2da61538d6a))
* **ui:** paint.net blurs in Image mode's Blurs submenu (AC-01, AC-02, AC-12, AC-15) ([ec36c78](https://github.com/amigo-labs/amigo-pincel/commit/ec36c78c75add17e8f3484fb7cc172584333827e))
* **ui:** paint.net distortions in the Distort submenu (AC-01, AC-02, AC-15) ([f49ed76](https://github.com/amigo-labs/amigo-pincel/commit/f49ed763a7085d2a4acaa8c440519da47bc74b17))
* **ui:** paint.net effect menus per mode with new parameter kinds (AC-01 – AC-07, AC-09 – AC-15) ([a0b314e](https://github.com/amigo-labs/amigo-pincel/commit/a0b314e64f8ba04a685fc6a4e0b846c25a7a67aa))
* **ui:** paint.net render effects in the Render submenu (AC-01, AC-02, AC-12) ([def5de7](https://github.com/amigo-labs/amigo-pincel/commit/def5de7b3b0a38fe70c1714727fd2a4cb954ae02))
* **ui:** Pencil Shading ink with a Shift+click palette ramp (shading-ink AC-01, AC-02, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-14) ([41a4804](https://github.com/amigo-labs/amigo-pincel/commit/41a4804228a3b238a5672350527e6c0432183766))
* **ui:** pixel grid and custom grid in Pixel mode (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-11, AC-12, AC-14, AC-16) ([00412da](https://github.com/amigo-labs/amigo-pincel/commit/00412daf04004f48c18162ccdb622d4bab7d0fbf))
* **ui:** rebind shortcuts in the Keyboard Shortcuts dialog (AC-01 – AC-06, AC-12, AC-13) ([7c9373c](https://github.com/amigo-labs/amigo-pincel/commit/7c9373ccab2c945c3b22480a276abce4280beb75))
* **ui:** recognize a dropped file as the file of an open tab (AC-03, AC-19) ([bd2cbbf](https://github.com/amigo-labs/amigo-pincel/commit/bd2cbbf621e591921fbf6a11d1143a48c54156ad))
* **ui:** resizable docked sidebar with a splitter, width kept per mode (AC-07 – AC-11) ([852277d](https://github.com/amigo-labs/amigo-pincel/commit/852277da3d052b1df040297357ac77a59257f619))
* **ui:** respect reduce motion and add View ▸ Reduce Motion (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-09, AC-10, AC-11, AC-12, AC-13) ([1c894be](https://github.com/amigo-labs/amigo-pincel/commit/1c894becf578c0192c8ac09c146af7fe7dbb2e83))
* **ui:** Scale choice for PNG frame and sprite-sheet export (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11) ([0c62d3c](https://github.com/amigo-labs/amigo-pincel/commit/0c62d3c753052b43ef36895f9b5dfdfa32ca4ca1))
* **ui:** symmetry and tiled mode for the Pixel-mode Pencil and Eraser (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16, AC-17, AC-18, AC-19, AC-20, AC-21, AC-22) ([da8bbbc](https://github.com/amigo-labs/amigo-pincel/commit/da8bbbc3e2abf33274624c1399444a6375aa1424))
* **website:** scroll the comparison inside its wrapper on phones with Pincel kept in view (website-comparison AC-09) ([982bcaa](https://github.com/amigo-labs/amigo-pincel/commit/982bcaaf3ca19c77c359160a030bf318d4330ed2))


### Bug Fixes

* **pincel-agent:** list the outline effect with a hex colour parameter (AC-12) ([0120e3c](https://github.com/amigo-labs/amigo-pincel/commit/0120e3cf8fb90e5f5551a9cc17fe2572eee04629))
* **pincel-core:** keep slice center and pivot relative on canvas changes ([57a2c6c](https://github.com/amigo-labs/amigo-pincel/commit/57a2c6c80b2d343c1569173bf44617ed40feef2f))
* **pincel-core:** stamping on a frame without a grid creates it (AC-21) ([384f958](https://github.com/amigo-labs/amigo-pincel/commit/384f958fa03c2253aaf55261f4cefdd54e1f1b23))
* **pincel-effects:** keep transparent pixels' colour out of distort and reduce noise (AC-18) ([dc153ad](https://github.com/amigo-labs/amigo-pincel/commit/dc153adef1c3ecc0e0af71712d9a4a5689c6d91a))
* **pincel-effects:** list outline once in EFFECT_NAMES after the PR [#123](https://github.com/amigo-labs/amigo-pincel/issues/123) merge ([428cac5](https://github.com/amigo-labs/amigo-pincel/commit/428cac5f6a3d916849f1edd0c2e7cd0c816b911a))
* **pincel-effects:** read the radial blur centre as a fraction of the image ([3fe6b2c](https://github.com/amigo-labs/amigo-pincel/commit/3fe6b2c5966f43131d9f709a110f9f001ea4416f))
* **pincel-effects:** read the radial blur centre as a fraction of the image ([57b63a2](https://github.com/amigo-labs/amigo-pincel/commit/57b63a221c72070197fe5b4d6286b4cabd179c12))
* **pincel-effects:** render Turbulence from gradient noise ([7bf710e](https://github.com/amigo-labs/amigo-pincel/commit/7bf710ea652a9cd1706704c1d9f63af128944a7c))
* **pincel-image-core:** add_layer keeps the layer tree intact ([2bc43a5](https://github.com/amigo-labs/amigo-pincel/commit/2bc43a56a982a095254939eeacdcb23112183616))
* **pincel-image-core:** PSD fill opacity scales the layer's opacity (AC-01) ([e1ead8e](https://github.com/amigo-labs/amigo-pincel/commit/e1ead8e1422391eb3b6060a88acf60656d6ae096))
* **pincel-wasm:** a new tileset stores the transparent tile 0 (AC-01, AC-04) ([734f76d](https://github.com/amigo-labs/amigo-pincel/commit/734f76d79eefee4e01e8aabe92a6549bcc311b1f))
* reconcile effect-panel AC-17 after integrating feat/clipboard-offer ([1d40a74](https://github.com/amigo-labs/amigo-pincel/commit/1d40a74efb7d9e7c5f84b341a294b8440e1ddf52))
* reconcile the effect-menu loading test after integrating feat/clipboard-offer ([9e0ef7d](https://github.com/amigo-labs/amigo-pincel/commit/9e0ef7d356e541e2908c84b0ce15f6d2c96f0c6f))
* reconcile the tool flyout with document tabs after integrating feat/tool-variant-flyout ([306922f](https://github.com/amigo-labs/amigo-pincel/commit/306922f20241d51a0aaa5a433b28c6e334aa1dac))
* **tauri:** drop the test-only labels_for so clippy passes ([e9399f7](https://github.com/amigo-labs/amigo-pincel/commit/e9399f71c8d129f98eb48d45dbf369ecacb60033))
* **tauri:** route current_labels through labels_for ([35c5fb1](https://github.com/amigo-labs/amigo-pincel/commit/35c5fb1a0bcfa15f6c2a65fc0c5ca6f9175c8832))
* **ui:** a refused slice size snaps back to the stored value (AC-10) ([4b0916f](https://github.com/amigo-labs/amigo-pincel/commit/4b0916f2e905ca9eb650f7330b789dfd0a449e74))
* **ui:** apply Select ▸ Expand… / Contract… in Pixel mode ([5dd2afe](https://github.com/amigo-labs/amigo-pincel/commit/5dd2afe33a6cbb92ffdc2713daddc1f0509fc0ee))
* **ui:** clamp an out-of-range New size instead of blocking Create (app-shell AC-06) ([0c3c7e0](https://github.com/amigo-labs/amigo-pincel/commit/0c3c7e0176ad2e80512e74e891edb7cd93ee9387))
* **ui:** count a Pixel inline layer rename as an undo step in the UI (pixel-layers-frames AC-08) ([d832ddf](https://github.com/amigo-labs/amigo-pincel/commit/d832ddf6217f7a3f85d1091fe9aa50e42a34565d))
* **ui:** draw Image-mode selection ants on device pixel centres ([08d3a76](https://github.com/amigo-labs/amigo-pincel/commit/08d3a76740bd2de77ed38671e6e383505ba48796))
* **ui:** drop the Image gesture in flight when a new tab opens ([47aba67](https://github.com/amigo-labs/amigo-pincel/commit/47aba67dc2e4f88766fe6342fa98b96a67a71146))
* **ui:** Enter on a menu entry or panel button keeps its own meaning while an effect is open (AC-04, AC-05) ([f61f898](https://github.com/amigo-labs/amigo-pincel/commit/f61f898d0fca48f85e794b1bf5ce33adaa3c756f))
* **ui:** find "…" effect entries by their English name in the command palette ([c767961](https://github.com/amigo-labs/amigo-pincel/commit/c767961e7f584e129db22fc2857c3e33980d54d9))
* **ui:** fit a new Pixel document to the stage its panels leave (pen-input AC-11) ([3626790](https://github.com/amigo-labs/amigo-pincel/commit/362679015e507f831226f39c0c196124ff874f5d))
* **ui:** focus the inline layer rename field when it opens (AC-08) ([0f33109](https://github.com/amigo-labs/amigo-pincel/commit/0f33109d3fbd9f9af4bcf4e9a8100ae592c756de))
* **ui:** keep a focused size field selected when the clipboard prefill lands ([92418b5](https://github.com/amigo-labs/amigo-pincel/commit/92418b58180313d9567906d75022a76dd3c2df4f))
* **ui:** keep native-menu items from the editor while the unsaved prompt is open (AC-02, AC-07, AC-08) ([439e7b2](https://github.com/amigo-labs/amigo-pincel/commit/439e7b252a37fca26a89c35489015cfee426fbbb))
* **ui:** keep the focus in the drop dialog when Add as layer is unavailable ([0c5cd6d](https://github.com/amigo-labs/amigo-pincel/commit/0c5cd6d9207107149a23d8c8dc1b657f7fe0cc3c))
* **ui:** keep the Pixel document unsaved when the save picker is cancelled ([3c795a5](https://github.com/amigo-labs/amigo-pincel/commit/3c795a511f6532ffefe6bf19263714324df4d7eb))
* **ui:** keep the shell starting when the window API throws under a Tauri 1 bridge ([7b505ba](https://github.com/amigo-labs/amigo-pincel/commit/7b505baab3896eb7e4b95972397e490d211b02c4))
* **ui:** name an unknown file opened from the Pixel editor (AC-12) ([638f441](https://github.com/amigo-labs/amigo-pincel/commit/638f4410a135277eae7b11f8c33e9cd8a53b6465))
* **ui:** Preview off hides the preview of a parameterless effect (AC-03) ([3fffeb3](https://github.com/amigo-labs/amigo-pincel/commit/3fffeb3a992f59ab1d6b2e47a3b6de1a6da16d25))
* **ui:** refuse a drop under the unsaved-changes prompt with a notice ([b828d90](https://github.com/amigo-labs/amigo-pincel/commit/b828d905ca623b22465bceeec1c628c1884b560f))
* **ui:** report an effect refused on a locked Image layer (AC-12) ([59fbe35](https://github.com/amigo-labs/amigo-pincel/commit/59fbe353e526da8b176fcb18f4924dfc35e8d14b))
* **ui:** route an Image-mode recent from the Pixel editor's Open Recent (app-shell AC-09) ([b0317e3](https://github.com/amigo-labs/amigo-pincel/commit/b0317e3bb03725be41e5203f161a39d8ab4942ec))
* **ui:** show the kept duration after a rejected frame duration entry (pixel-layers-frames AC-31) ([387fadd](https://github.com/amigo-labs/amigo-pincel/commit/387faddb3694fd1a6209b6f900cf1dc5f0df0ef8))
* **ui:** show the shell notice when the Pixel editor cannot read a file (file-access AC-14, AC-16) ([51ec205](https://github.com/amigo-labs/amigo-pincel/commit/51ec205bf36cfab5b562eb6a73a8a7af6b9b2935))
* **ui:** slice 9-patch center and pivot relative to the bounds (AC-09, AC-12, AC-13) ([f9abb31](https://github.com/amigo-labs/amigo-pincel/commit/f9abb3105086eab1174c0cc8a3d94199aaa8f20b))
* **ui:** the Add tileset form always opens with its defaults (AC-02) ([685edba](https://github.com/amigo-labs/amigo-pincel/commit/685edba3e9a3c40e159d4ef58dd954371d51f8e4))
* **ui:** the Add tileset form reports every bad tile size itself (AC-03) ([ef9bea6](https://github.com/amigo-labs/amigo-pincel/commit/ef9bea66ee41c25f94af5cbd72bb2b643461ed10))
* **ui:** the Image editor keeps the native Open Recent submenu current (AC-19) ([65c13c7](https://github.com/amigo-labs/amigo-pincel/commit/65c13c7f1a5a4becd359cbdb56e060cd56d91373))
* **ui:** the Tilesets panel hides the empty tile 0 (AC-07) ([83516eb](https://github.com/amigo-labs/amigo-pincel/commit/83516eb1c67b09f6247d147965b92a4040b1ae5c))
* **ui:** Tilesets panel edits refresh the undo state (AC-01, AC-04) ([3f28ab2](https://github.com/amigo-labs/amigo-pincel/commit/3f28ab25c540f43fe3d5fc60e7dd412a3bba4ade))
* **ui:** Tilesets panel strings go through t() (AC-01, AC-04, AC-06) ([0f15aa8](https://github.com/amigo-labs/amigo-pincel/commit/0f15aa899b96a4ee845987a64e3d937ed1b5bfe3))
* **ui:** translate the Slices panel's field labels and row texts (AC-11) ([0626569](https://github.com/amigo-labs/amigo-pincel/commit/0626569dfb025bcb01de4f0aa38fd8222198e848))
* **ui:** translate the sprite sheet dialog's grid line (AC-16) ([3830dde](https://github.com/amigo-labs/amigo-pincel/commit/3830dde6aa186178c225b0493a50dcdbcc8a37da))
* **website:** call Aseprite source-available and date Piskel's last release (website-comparison AC-06, AC-07) ([973fdd2](https://github.com/amigo-labs/amigo-pincel/commit/973fdd2cc134925a3c1e296113f7148080e61019))
* **website:** set `<html lang>` on a client-side language switch (website AC-01 – AC-05) ([75bf1bf](https://github.com/amigo-labs/amigo-pincel/commit/75bf1bf9879e80a3b5ce20db7f49266f5d7b9611))


### Performance Improvements

* **pincel-image-core:** bench a 4K brush stroke presented from the cache (AC-02) ([c656e67](https://github.com/amigo-labs/amigo-pincel/commit/c656e67cf66ee2ef519ef05318535b7a1ed9cd0e))
* **pincel-image-core:** cached composite with a downscaled pyramid (AC-01, AC-05, AC-06, AC-07) ([69ecefd](https://github.com/amigo-labs/amigo-pincel/commit/69ecefdffecb4ffc197b79beb811c71730867338))
* **pincel-image-core:** compose without powf, byte for byte (AC-01, AC-02, AC-04) ([ba53299](https://github.com/amigo-labs/amigo-pincel/commit/ba53299945d0e2128f658d7a9f7f620ec3b6285f))
* **pincel-image-core:** division-free pyramid filter for equal alphas (AC-07) ([018e2b7](https://github.com/amigo-labs/amigo-pincel/commit/018e2b7901a23f244c1678d5c0273d557ec4e482))
* **pincel-image-wasm:** composite_region presents from the render cache (AC-01, AC-05, AC-06, AC-07) ([b97a773](https://github.com/amigo-labs/amigo-pincel/commit/b97a773dbb3d5f60d1dd0685f7dfbaeb2c50fa1a))

## [1.6.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.5.0...v1.6.0) (2026-11-02)


### Features

* integrate the Vectorizer (pincel-trace + File ▸ Export ▸ SVG…) ([9a6ed31](https://github.com/amigo-labs/amigo-pincel/commit/9a6ed3169e8f93a65ad442e48b58f39bfd88bd32))
* **pincel-image-wasm:** vectorizer over the composite for SVG export ([f2800ea](https://github.com/amigo-labs/amigo-pincel/commit/f2800ea302ae9e63ad75454f9ec906f8d9ed415f))
* **pincel-trace:** pixel-exact mode and the SVG document (AC-12 – AC-14, AC-16, AC-17, AC-20) ([8c9286d](https://github.com/amigo-labs/amigo-pincel/commit/8c9286d4bf739fc0165976d494936c99248a3946))
* **pincel-trace:** port the Vectorizer core (AC-02 – AC-11, AC-15, AC-18, AC-19) ([4ae67ad](https://github.com/amigo-labs/amigo-pincel/commit/4ae67adc8c51030ba3eb7e28f09df643d8f95dbc))
* **pincel-trace:** read trace options from a flat wire vector ([22c7d62](https://github.com/amigo-labs/amigo-pincel/commit/22c7d62facee7cae96d7a08f3d0426e28e233507))
* **pincel-wasm:** Vectorizer over a composed frame for SVG export ([aeb4797](https://github.com/amigo-labs/amigo-pincel/commit/aeb4797c39796cff202bcf4ff5312ddd78920a39))
* **tauri:** Export as SVG… in the native File menu (AC-10) ([0abc73a](https://github.com/amigo-labs/amigo-pincel/commit/0abc73a12f5f18ab38d3e63ee9218ed33964d1dd))
* **ui:** File ▸ Export ▸ SVG… in both editors (AC-01 – AC-09, AC-11) ([092dc60](https://github.com/amigo-labs/amigo-pincel/commit/092dc60020a70f69af05f3cd60709b9eb5cb73ab))
* **ui:** paste the clipboard image as a new document ([2350eca](https://github.com/amigo-labs/amigo-pincel/commit/2350eca304f9cb6fe6326f1bdff7ba7e9a4705c9))
* **ui:** paste the clipboard image as a new document (AC-22, AC-23, AC-24, AC-25, AC-26) ([f323697](https://github.com/amigo-labs/amigo-pincel/commit/f323697ef7773979f8be97372984990f911f8482))
* **ui:** start image selections on the stage, deselect by click and Escape (AC-29, AC-30, AC-31, AC-32) ([9b852d4](https://github.com/amigo-labs/amigo-pincel/commit/9b852d4916770df28d1902556f4abc8d460e8408))
* **ui:** start pixel selections on the stage beside the canvas (AC-01, AC-04, AC-07, AC-09, AC-12, AC-17, AC-18, AC-21, AC-22, AC-23, AC-24) ([a554af6](https://github.com/amigo-labs/amigo-pincel/commit/a554af6d0bf3866450326c4a68168ce8628cb56e))
* **ui:** start selections on the stage beside the canvas ([14991b4](https://github.com/amigo-labs/amigo-pincel/commit/14991b4a7092cccd036eeaaacc161892f25c2806))


### Bug Fixes

* **pincel-trace:** reserve outline-sized buffers fallibly (AC-18) ([87ecaa4](https://github.com/amigo-labs/amigo-pincel/commit/87ecaa4548e835a4f210e1f79ac483a1f36723ea))
* **pincel-trace:** share cached quantizations instead of copying them (AC-18) ([40498e5](https://github.com/amigo-labs/amigo-pincel/commit/40498e5bf61bcf99d841540ea4cafd4e1b21e0df))
* **pincel-trace:** use as_chunks for RGBA pixels (clippy 1.99) ([cee3a28](https://github.com/amigo-labs/amigo-pincel/commit/cee3a28bb92a7526b818588e0a09355650a6960e))
* **tauri:** stop the PWA service worker from freezing the desktop UI ([4c15c89](https://github.com/amigo-labs/amigo-pincel/commit/4c15c89a29e449195833b6fc0f125e80f0a349cc))
* **tauri:** stop the PWA service worker from freezing the desktop UI ([a321a03](https://github.com/amigo-labs/amigo-pincel/commit/a321a03235eb48d3e02d9a140ebdfde956f4e8c0))
* **ui:** drop a pixel gesture on pointercancel instead of committing it (AC-09, AC-21, AC-22, AC-23, AC-24, AC-29, AC-30, AC-31, AC-32) ([501fb42](https://github.com/amigo-labs/amigo-pincel/commit/501fb4260b0f5269e1ada2abf38a5cbe9031fd3f))
* **ui:** keep the SVG export dialog modal for pointer and keyboard ([866fe91](https://github.com/amigo-labs/amigo-pincel/commit/866fe91eeac2bee8f274c1c49f302e6952ba8a52))
* **ui:** let a key right after a marquee release see the new selection ([e756741](https://github.com/amigo-labs/amigo-pincel/commit/e75674126ec0696b4129a378f19c15c68b68015a))
* **ui:** march Image-mode selection ants instead of blinking the outline ([3bcb5ca](https://github.com/amigo-labs/amigo-pincel/commit/3bcb5ca2ef0bda3c042465dbbe389f0cc3065f5c))
* **ui:** march Image-mode selection ants instead of blinking the outline ([c8b134c](https://github.com/amigo-labs/amigo-pincel/commit/c8b134c10285ea6503baab405da462816becac76))
* **ui:** snap both image-mode marquee corners to the pixel grid ([1819fda](https://github.com/amigo-labs/amigo-pincel/commit/1819fda248e08602b654fb2704773ff78b862e8d))
* **ui:** tie the SVG tracer to the document it traces ([7257535](https://github.com/amigo-labs/amigo-pincel/commit/72575352dde5a76e5c6c973c84116ad48d43b1ec))

## [1.5.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.4.0...v1.5.0) (2026-10-30)


### Features

* add skills for feature specification and verification ([6526967](https://github.com/amigo-labs/amigo-pincel/commit/6526967d4e9afa5dbd4158800aeab8c22ed5ba09))
* copy, cut and paste in both modes (copy-cut-paste) ([28b8cc5](https://github.com/amigo-labs/amigo-pincel/commit/28b8cc54f573669ab38cb3c88b1cce49412ba8d7))
* **perf:** benchmarks, budgets and nightly performance workflow ([4229ec7](https://github.com/amigo-labs/amigo-pincel/commit/4229ec72edc2cf5471d1a64409a5b0c7e238b7d6))
* **pincel-agent:** add pincel://guide and pincel://effects resources (AC-17, AC-18) ([a2649fc](https://github.com/amigo-labs/amigo-pincel/commit/a2649fc1f8ba7e2b63eab0a017ce768f0f8ac909))
* **pincel-agent:** add the Pixel agent tool layer (AC-02..AC-33, AC-43..AC-46) ([132926b](https://github.com/amigo-labs/amigo-pincel/commit/132926be6ac5eff75800583825b63986c570e5e5))
* **pincel-agent:** Image documents over MCP (mcp-headless-image) ([5b65b7d](https://github.com/amigo-labs/amigo-pincel/commit/5b65b7df2131aee1aa73dd940cb25144b2f04df3))
* **pincel-agent:** open, edit, save and export Image documents (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-16, AC-19, AC-20, AC-21) ([a175f1a](https://github.com/amigo-labs/amigo-pincel/commit/a175f1af5fe661ecc887324c593fddbe5c794d3a))
* **pincel-core:** add CommandBatch and PutCel commands (AC-09) ([f10b887](https://github.com/amigo-labs/amigo-pincel/commit/f10b887afab2e4c3356e3efa3e7612b23a5df490))
* **pincel-core:** add copy_pixels and the PasteLayer command (AC-02, AC-04, AC-05, AC-07, AC-11, AC-18, AC-19) ([842fa4c](https://github.com/amigo-labs/amigo-pincel/commit/842fa4ce741d6cc8f2bb04c31f49886086aaa9c2))
* **pincel-core:** add PatchCel and AddEmptyCel dirty-rect commands (AC-09) ([05d67e8](https://github.com/amigo-labs/amigo-pincel/commit/05d67e81334f23ed1faa8081acb2be66695ef130))
* **pincel-core:** add PencilStroke command with brush sizes and pixel-perfect (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-08, AC-09, AC-10) ([6a7e87e](https://github.com/amigo-labs/amigo-pincel/commit/6a7e87eea8461e3f0e564be9faaa992112066e74))
* **pincel-core:** animated export as APNG and GIF (animated-export) ([11545c5](https://github.com/amigo-labs/amigo-pincel/commit/11545c5b1ad3aa507530aea336713005507c90b1))
* **pincel-core:** export animations as APNG with tag direction and scale (AC-03, AC-04, AC-05, AC-07, AC-08, AC-09) ([74d5722](https://github.com/amigo-labs/amigo-pincel/commit/74d5722d6fe0677138fffe0db0e70198cb619386))
* **pincel-core:** export animations as GIF and describe them for the dialog (AC-12, AC-13, AC-16, AC-17) ([ad1cd36](https://github.com/amigo-labs/amigo-pincel/commit/ad1cd367accdc96fd046173018ff3ea4f8ed54dc))
* **pincel-core:** scale PencilStroke stamps by pen pressure (AC-01, AC-02, AC-04, AC-05, AC-06, AC-09, AC-10) ([3e205f0](https://github.com/amigo-labs/amigo-pincel/commit/3e205f028a21d5cca1eaea31bab6dcdac2f47c13))
* **pincel-image-core:** add copy_selection and PasteLayer::at (AC-02, AC-03, AC-04, AC-05, AC-09, AC-10) ([a08d803](https://github.com/amigo-labs/amigo-pincel/commit/a08d80395a38c15db020f76715e82eb30bb9a56b))
* **pincel-image-core:** add lossless TIFF encoder (AC-01) ([6f20017](https://github.com/amigo-labs/amigo-pincel/commit/6f20017f9c0935e418929c5c0a552db5b11973a8))
* **pincel-image-wasm:** add copy_selection, cut_selection, image_key and a paste position (AC-02, AC-03, AC-04, AC-05, AC-06, AC-09, AC-10, AC-21) ([a4f8a7f](https://github.com/amigo-labs/amigo-pincel/commit/a4f8a7f58b4a3362be42efe97e7fa939e015bc1e))
* **pincel-image-wasm:** export the composite as BMP and TIFF (AC-01) ([44f2d55](https://github.com/amigo-labs/amigo-pincel/commit/44f2d5594eb2ae9d19ba3d77f758108b1c25e26e))
* **pincel-mcp:** add the stdio MCP server with workspace sandbox (AC-01, AC-34..AC-40) ([c00ff71](https://github.com/amigo-labs/amigo-pincel/commit/c00ff713b9fc41080181a1da7bf108a8d39c3174))
* **pincel-mcp:** headless MCP server for Pixel documents ([773b296](https://github.com/amigo-labs/amigo-pincel/commit/773b296fa75ee59999bc148cf799bb65a53abc75))
* **pincel-mcp:** serve resources with their MIME type and describe Image mode (AC-17, AC-18) ([50acfa0](https://github.com/amigo-labs/amigo-pincel/commit/50acfa0f3a984c02b3625a65c37d9ba4647192be))
* **pincel-wasm:** add beginStroke / strokeTo gesture API for Pencil and Eraser (AC-01, AC-03, AC-04, AC-05, AC-06, AC-08, AC-14) ([ec6b135](https://github.com/amigo-labs/amigo-pincel/commit/ec6b13528fbc078d2a5a931aeceb240a11423b50))
* **pincel-wasm:** add copySelection, cutSelection, pasteLayer and imageKey (AC-02, AC-04, AC-05, AC-06, AC-07, AC-11, AC-12, AC-18, AC-19, AC-20, AC-21) ([3e4198d](https://github.com/amigo-labs/amigo-pincel/commit/3e4198d1c78e077d6d4a961c026fb6b2e0c1f292))
* **pincel-wasm:** add optional pressure argument to strokeTo (AC-01, AC-04, AC-05, AC-09, AC-10) ([f8f6a56](https://github.com/amigo-labs/amigo-pincel/commit/f8f6a56e853d5e0a3abb348f6e7ac550f3cc6b2c))
* **pincel-wasm:** expose exportAnimation and animationInfo (AC-18) ([74d3e7f](https://github.com/amigo-labs/amigo-pincel/commit/74d3e7f2c2232eaad6587f9f274c253b2005ee71))
* **pixel:** pen pressure scales the Pencil and Eraser (pixel-pressure) ([4610e12](https://github.com/amigo-labs/amigo-pincel/commit/4610e12fc5bf6beaf6b90604a266281377e6fc67))
* **pixel:** pencil brush sizes 1-8, round shape, pixel-perfect and Shift constrain ([1afe8df](https://github.com/amigo-labs/amigo-pincel/commit/1afe8df514d8a76642a2426796cc2c2e357adcd3))
* **tauri:** add Export Animation to the native File menu ([44af9fb](https://github.com/amigo-labs/amigo-pincel/commit/44af9fbab1ad1e2860362149e0e314ce5f77dc18))
* **ui:** add File &gt; Export &gt; Animation dialog for APNG and GIF (AC-01, AC-02, AC-06, AC-10, AC-11, AC-14, AC-15, AC-18, AC-19) ([e7cee29](https://github.com/amigo-labs/amigo-pincel/commit/e7cee295c2420ec104fb2e0e7ff6b8203d031dcd))
* **ui:** ask to flatten layered documents on Image-mode Save (AC-02, AC-03, AC-04, AC-05, AC-06, AC-13) ([d88e864](https://github.com/amigo-labs/amigo-pincel/commit/d88e8649d2202019c498bc25068cf205934f8666))
* **ui:** brush size, shape, pixel-perfect and Shift constrain for the Pixel pencil (AC-01, AC-03, AC-07, AC-08, AC-10, AC-11, AC-12, AC-13, AC-15, AC-16, AC-17) ([f730687](https://github.com/amigo-labs/amigo-pincel/commit/f730687f1fc898bcbec85873b94de19370697497))
* **ui:** Cut, Copy, Copy Merged and Paste in both modes (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15, AC-18, AC-19, AC-20, AC-21) ([ee56179](https://github.com/amigo-labs/amigo-pincel/commit/ee56179f58b2b62705c739b94bc06cb7172c650b))
* **ui:** offer every writable format in Image-mode Save As (AC-09, AC-10, AC-14) ([1558134](https://github.com/amigo-labs/amigo-pincel/commit/1558134c6b8183b5ef013dc5ff12b60ee95b380e))
* **ui:** save Image-mode documents back to their original format (AC-01, AC-07, AC-08, AC-11, AC-12) ([9ba032c](https://github.com/amigo-labs/amigo-pincel/commit/9ba032c3f9ce9445ecd80d6c72812dd21e4f32b0))
* **ui:** save Image-mode documents back to their original format (save-original-format) ([cebd982](https://github.com/amigo-labs/amigo-pincel/commit/cebd982c9421e2a90bc88d78812bd6d578772778))
* **ui:** scale the Pixel-mode Pencil and Eraser with pen pressure (AC-03, AC-05, AC-07, AC-08, AC-09) ([a8fc24e](https://github.com/amigo-labs/amigo-pincel/commit/a8fc24ec367fb56c20e628e1ef63ad1193c4543f))


### Bug Fixes

* **aseprite-writer:** emit tileset chunks before the layers that reference them ([dfd4fde](https://github.com/amigo-labs/amigo-pincel/commit/dfd4fde9a2d72163ddb1a1dbda2a668af9026441))
* **pincel-agent:** check every export.sheet output path before writing (AC-34) ([1e0ae49](https://github.com/amigo-labs/amigo-pincel/commit/1e0ae49b68c4885b7c96e693ce7b45fcd5c1db38))
* **pincel-agent:** store only the changed rectangle per pixel edit (AC-09) ([bd859a1](https://github.com/amigo-labs/amigo-pincel/commit/bd859a1e8f5b93d36fbdc1b824ed5ab1a9cd7da9))
* **pincel-core:** read and write tile flip bits the way Aseprite does ([353439e](https://github.com/amigo-labs/amigo-pincel/commit/353439e92b4becf55bd78e9a3ddee8dd1d086f46))
* **pincel-core:** read and write tile flip bits the way Aseprite does ([59b59af](https://github.com/amigo-labs/amigo-pincel/commit/59b59af12d3976234bed9aca4fb4312734e9269a))
* **pincel-image-core:** escape NUL bytes in the TIFF magic test ([9dd6929](https://github.com/amigo-labs/amigo-pincel/commit/9dd6929b597ee68f3db3a411f77f09bb18fa6a73))
* **pincel-image-core:** Soft Light follows W3C Compositing-1, not Pegtop (AC-02) ([085b900](https://github.com/amigo-labs/amigo-pincel/commit/085b900cca543167b7c51fa17614b7a739ef8b28))
* **pincel-image-core:** Soft Light follows W3C; close image-editing evidence gaps ([e57ab0d](https://github.com/amigo-labs/amigo-pincel/commit/e57ab0dc9c8800eda90a248ec4289c575a6e620d))
* **ui:** ask to flatten when the image's only layer is hidden ([f072774](https://github.com/amigo-labs/amigo-pincel/commit/f0727748f297701a3f037de74acec61596efa62c))
* **ui:** cancel a pending Image-mode Save prompt when the editor unmounts ([a7d52ea](https://github.com/amigo-labs/amigo-pincel/commit/a7d52eaa28bb2679f78d9fda47084205f3e5fb81))
* **ui:** keep a recovered image's detected save format in its snapshot ([35ffcc4](https://github.com/amigo-labs/amigo-pincel/commit/35ffcc446f9ef4fa45f5acf8e2d09a272b8f2056))
* **ui:** keep the layered snapshot current after a flattening Save ([fbedd4a](https://github.com/amigo-labs/amigo-pincel/commit/fbedd4ac958afa384804a63f19fc9531cfa65920))
* **ui:** lock the Shift pencil direction until Shift is released (AC-07, AC-17) ([cddb39b](https://github.com/amigo-labs/amigo-pincel/commit/cddb39ba604d0e32857b23212839efd74451f9ca))
* **ui:** route every e2e path into an editor through waitForEditor ([e282fe0](https://github.com/amigo-labs/amigo-pincel/commit/e282fe044fc9788f5e2e9a626aef335cc0dc6856))
* **ui:** route every e2e path into an editor through waitForEditor ([a5b71e4](https://github.com/amigo-labs/amigo-pincel/commit/a5b71e47f3a4ce4196781b26bca46c956fba1adf))
* **ui:** skip off-canvas pixels of a pencil or eraser stroke silently (AC-04) ([9e8930d](https://github.com/amigo-labs/amigo-pincel/commit/9e8930d41293e6296fd5b1065e1e2baafa516b9b))
* **ui:** wait for editor readiness before e2e input ([42c3e2b](https://github.com/amigo-labs/amigo-pincel/commit/42c3e2bacb563d7002beb8ede9e3a37df95dad93))
* **ui:** wait for editor readiness before e2e input ([8a6def7](https://github.com/amigo-labs/amigo-pincel/commit/8a6def77399a1a81505d2f40ba4c1d4aef215fee))

## [1.4.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.3.1...v1.4.0) (2026-10-28)


### Features

* **pincel-image-core:** add PasteLayer command ([acd8c72](https://github.com/amigo-labs/amigo-pincel/commit/acd8c72f4c885371ef69dd582de92934236e119c))
* **pincel-image-wasm:** export paste_layer ([87add73](https://github.com/amigo-labs/amigo-pincel/commit/87add7311745239e297e69d66ed0b4bc5dfbd763))
* **ui:** open a new document on startup from a ticked New card ([d603c0a](https://github.com/amigo-labs/amigo-pincel/commit/d603c0ad97ed2ed2a1f62154e4be01e323b9e1b1))
* **ui:** paste a clipboard image as a new layer in Image mode ([3cc61cc](https://github.com/amigo-labs/amigo-pincel/commit/3cc61cc462a37bfcf8c751e906e5927ec5da38d8))
* **ui:** redraw the Smudge, Gradient and Stamp icons ([3952751](https://github.com/amigo-labs/amigo-pincel/commit/39527513f163054e12217bca8e5b2b0042d09f57))
* **ui:** separate the tool rail into sections ([52562c9](https://github.com/amigo-labs/amigo-pincel/commit/52562c964cedfdbaa2349ba0f7274af508b17c2f))
* **ui:** size a new image from the image on the clipboard ([5843050](https://github.com/amigo-labs/amigo-pincel/commit/5843050e081290055e8cd47ee5011cc66adac18a))
* **ui:** startup document, clipboard image size and paste with canvas expand ([bbbf685](https://github.com/amigo-labs/amigo-pincel/commit/bbbf6853b5c02f8af870630ddacfdb7a5b1aadca))


### Bug Fixes

* **pincel-image-core:** keep the paste's layer index across redo ([bce05bc](https://github.com/amigo-labs/amigo-pincel/commit/bce05bce02f57c7c08b9ad1f4dcbd7090a288828))
* **ui:** never store a non-finite startup document size ([4fefcf1](https://github.com/amigo-labs/amigo-pincel/commit/4fefcf11af8580ee858f64f4bb0d1ff5f4bfdaa6))
* **ui:** paste only into the document it was asked for ([167d521](https://github.com/amigo-labs/amigo-pincel/commit/167d521568912dcc73a844173b041b7792f52559))

## [1.3.1](https://github.com/amigo-labs/amigo-pincel/compare/v1.3.0...v1.3.1) (2026-10-27)


### Miscellaneous

* force the 1.3.1 release to validate the trusted publisher ([ef4d693](https://github.com/amigo-labs/amigo-pincel/commit/ef4d693b7836e678eaca15e18660b3c3f0e704d2))

## [1.3.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.2.0...v1.3.0) (2026-10-27)


### Features

* **pincel-image-core:** move the selected pixels, and the selection alone ([5575dcf](https://github.com/amigo-labs/amigo-pincel/commit/5575dcff9d43ec2430ae29ee6dea8f04aed09152))
* **pincel-image-core:** the Gradient tool ([66ededd](https://github.com/amigo-labs/amigo-pincel/commit/66ededdfa9925896c9cc7591345b55c4b26287bb))
* **pincel-image-core:** the Recolor and Smudge tools ([7412859](https://github.com/amigo-labs/amigo-pincel/commit/74128595e580594e9bbbce9cdc7d8deaa22dba17))
* **pincel-image-core:** the selection outline travels with its pixels ([5da1bcb](https://github.com/amigo-labs/amigo-pincel/commit/5da1bcb6574c8d1e20a6a9a8757be6cf00352906))
* **tauri:** updater commands for the in-app Help menu ([3b48369](https://github.com/amigo-labs/amigo-pincel/commit/3b48369e7e917e9ddb7b3ab9d0cb9ea983b2867d))
* **ui:** check for updates from the Help menu ([78247ae](https://github.com/amigo-labs/amigo-pincel/commit/78247ae3b4b93ca96fad47622b4433f11324f7d4))
* **ui:** expand, contract and feather the selection by a radius ([f57fc16](https://github.com/amigo-labs/amigo-pincel/commit/f57fc16824f50cf46c8358f4d45fa42c84d2e1e2))
* **ui:** File ▸ Open Recent in Image mode ([9e05daf](https://github.com/amigo-labs/amigo-pincel/commit/9e05daf83e7c7bc1f7e77f81b192d6d64bfeafe2))
* **ui:** Help menu with About Pincel ([785fa64](https://github.com/amigo-labs/amigo-pincel/commit/785fa64b6b4807cd195d227e8b07476a9c6c870b))
* **ui:** line, the four shapes, pan and zoom as Image-mode rail tools ([abbe448](https://github.com/amigo-labs/amigo-pincel/commit/abbe448d49a8e9d4501ac3dec0d3aff553c73a4e))
* **ui:** move the selected pixels, or the selection alone ([a9ea6da](https://github.com/amigo-labs/amigo-pincel/commit/a9ea6dac0318c28162d0c33b972cdbafb5d60c05))
* **ui:** the Gradient tool ([6036064](https://github.com/amigo-labs/amigo-pincel/commit/603606406e30a674792c328d6cd6b08e4e23dca7))
* **ui:** the Recolor and Smudge tools ([53a2c0f](https://github.com/amigo-labs/amigo-pincel/commit/53a2c0f6a13f2c7775ef95f79e8583be6b6a56eb))


### Bug Fixes

* **pincel-image-core:** carry Smudge and Recolor state across a drag ([9a3dc0e](https://github.com/amigo-labs/amigo-pincel/commit/9a3dc0ee5898ea1458d9d14e5c9d278b9ec76e42))
* **tauri:** install the native menu only on macOS ([046b87c](https://github.com/amigo-labs/amigo-pincel/commit/046b87c5b9cd3c1b65b8c7eff5192f7781869b7e))
* **tauri:** one menu bar per platform, and the Image-mode tools paint.net has ([95b9b18](https://github.com/amigo-labs/amigo-pincel/commit/95b9b180869c79ca8aa0d14ea64e66a4e9c4969e))
* **ui:** four defects from the review of the new tools ([e1bed0e](https://github.com/amigo-labs/amigo-pincel/commit/e1bed0e3242c4346f31732e805f31a349caf6453))
* **ui:** the About dialog holds the keyboard while it is open ([9574a2d](https://github.com/amigo-labs/amigo-pincel/commit/9574a2d6f2ff4dfaea0d77c06d3cb4a40eea21b6))

## [1.2.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.1.2...v1.2.0) (2026-10-27)


### Features

* **pincel-core:** HSL blend modes; LF line endings from feat/blend-modes ([32280da](https://github.com/amigo-labs/amigo-pincel/commit/32280da7969d1b9dd57716cb772c6aae730d70b1))
* **pincel-core:** render the Hue, Saturation, Color and Luminosity blend modes ([581218e](https://github.com/amigo-labs/amigo-pincel/commit/581218e21ba2637dd9e26ca5f8986af831f5ae3f))

## [1.1.2](https://github.com/amigo-labs/amigo-pincel/compare/v1.1.1...v1.1.2) (2026-10-27)


### Bug Fixes

* **tauri:** updater trusts the new amigo-labs signing key ([b3b9420](https://github.com/amigo-labs/amigo-pincel/commit/b3b942018b5984a4684dff2841daca9656baf066))
* **tauri:** updater trusts the new amigo-labs signing key ([80ae66c](https://github.com/amigo-labs/amigo-pincel/commit/80ae66c41c6764a4edbc2524c2f013b30333ba13))

## [1.1.1](https://github.com/amigo-labs/amigo-pincel/compare/v1.1.0...v1.1.1) (2026-10-27)


### Bug Fixes

* **tauri:** sign updates with the amigo-labs organization key ([f8e2ddc](https://github.com/amigo-labs/amigo-pincel/commit/f8e2ddc8300ea6a086a1f9b2a5c28b9b11cfb5bd))
* **tauri:** updater trusts the amigo-labs signing key ([15608a0](https://github.com/amigo-labs/amigo-pincel/commit/15608a05c1eb2d340a58826adf8adb924441b608))

## [1.1.0](https://github.com/amigo-labs/amigo-pincel/compare/v1.0.1...v1.1.0) (2026-10-26)


### Features

* desktop downloads on the website, New cards open in place on the start screen ([8d3e411](https://github.com/amigo-labs/amigo-pincel/commit/8d3e411e34403c37b54c7858046caab5763a8d93))
* **ui:** New cards on the start screen open in place ([f3eb37a](https://github.com/amigo-labs/amigo-pincel/commit/f3eb37a29c5696f5c3462a952f646efc6502dacd))

## [1.0.1](https://github.com/amigo-labs/amigo-pincel/compare/v1.0.0...v1.0.1) (2026-10-26)


### Bug Fixes

* **tauri:** Windows app icon as RGBA so the Windows build embeds it ([8fad095](https://github.com/amigo-labs/amigo-pincel/commit/8fad095293c8d6548ccbbf6c8c773faf1a11aa18))
* **tauri:** Windows app icon as RGBA so the Windows build embeds it ([fc84db9](https://github.com/amigo-labs/amigo-pincel/commit/fc84db951221f09abe759a1e3d55f18ff2ca8de1))

## [1.0.0](https://github.com/amigo-labs/amigo-pincel/compare/v0.1.9...v1.0.0) (2026-10-26)


### Features

* **release:** release-please, self-updater, setup and start-up as in amigo-blinzel ([2695564](https://github.com/amigo-labs/amigo-pincel/commit/269556452826ee235680fdae5adc13d594ef15ee))
* **release:** web and desktop installers in every release, published as one ([f24056e](https://github.com/amigo-labs/amigo-pincel/commit/f24056e0687e86ce7903c43a38cc451a5652a35b))
* **tauri:** setup and start-up behaviour as in amigo-blinzel ([50083e6](https://github.com/amigo-labs/amigo-pincel/commit/50083e65f40fce3da0790d94005c0f323582e992))
* **tauri:** silent self-updater, as in amigo-blinzel ([50d963a](https://github.com/amigo-labs/amigo-pincel/commit/50d963a7e08caa729cb71b8b02b9af04c7742b58))


### Bug Fixes

* **release:** point the Tauri CLI at src-tauri ([add5c95](https://github.com/amigo-labs/amigo-pincel/commit/add5c95d6c00a3d52328f619f3ee4b539217ffd1))
* **tauri:** keep the setup's Capabilities under the installer's own key ([ef95e22](https://github.com/amigo-labs/amigo-pincel/commit/ef95e22d4df63d1704e2bafb36bbd6ed759aff40))
* **tauri:** offer Default apps once, also across interactive upgrades ([10f70d8](https://github.com/amigo-labs/amigo-pincel/commit/10f70d814d108022dc4902573f99219b7d4bb0af))
* **tauri:** updater settings written whole and one at a time ([69cf888](https://github.com/amigo-labs/amigo-pincel/commit/69cf888a9ca0cba0b9b5df09c6230cd167cebbf1))

## Changelog

## Before 1.0.0 (v0.1.x pre-releases)

The v0.1.x pre-releases (one per push to `main`) built up the full
Phase 1 scope per
[`docs/specs/pincel.md`](docs/specs/pincel.md) §16.

### Added

- **Core document model** (`pincel-core`): Sprite, Layer, Frame, Cel,
  Palette, Tileset, Slice, Tag types with a `SpriteBuilder` API. Pure
  logic, no I/O, no platform dependencies.
- **Command bus + undo/redo** with a 100-deep linear history. Commands:
  `SetPixel`, `AddLayer`, `AddFrame`, `DrawLine`, `DrawRectangle`,
  `DrawEllipse`, `FillRegion`, `MoveSelectionContent`, `AddTileset`,
  `PlaceTile`, `SetTilePixel`, `AddSlice`, `RemoveSlice`, `SetSliceKey`.
- **`compose()`** for RGBA image and tilemap layers — viewport-windowed,
  zoom-aware, caller-owned scratch buffer, `dirty_hint` honored,
  per-command dirty-region reporting routed end-to-end into a Canvas2D
  sub-rect blit.
- **Aseprite codec.** Read (image layers + tilemaps + slices, via
  `aseprite-loader`) and write (header, layer, cel, palette, tags,
  tileset, tilemap cel, slice chunks). Round-trip tests cover image,
  tilemap, and 9-patch+pivot slice fixtures.
- **`aseprite-writer` crate** as a standalone, publishable Rust library
  for the v1.3 `.aseprite` format, dual-licensed MIT or Apache-2.0.
- **`pincel-wasm`** wasm-bindgen layer with the public API surface
  described in spec §17.5 — opens, paints, exports, saves
  `.aseprite` files from any JS host. Builds to `@amigo-labs/pincel`
  for npm.
- **UI tool set** (Svelte 5 + Tailwind 4): Pencil, Eraser, Eyedropper,
  Line, Rectangle (outline + fill), Ellipse (outline + fill), Bucket,
  Move (viewport pan + selection-content drag), Selection (Rect) with
  marching-ants overlay, Tilemap Stamp with grid + cell hover overlay,
  Slice tool with 9-patch and pivot editing.
- **Tileset Panel + Tile Editor modal** — add tilesets, add tiles,
  edit tile pixels with the full undo stack.
- **Slices Panel** — add / remove slices, edit bounds, toggle
  9-patch center, toggle pivot.
- **PWA** — service worker via `vite-plugin-pwa` `injectManifest`,
  manifest (standalone display, theme color, maskable icon).
- **File I/O** — File System Access API with in-place save-through and
  download fallback, recents (IndexedDB, 8-entry LRU), 30-second
  autosave snapshots with boot-time Recovery Dialog.
- **Tauri 2 desktop shell** — native open/save dialogs, native menu
  bar (File / Edit / View / Help) with platform accelerators, file
  associations for `.aseprite` / `.ase`, single-instance forwarding,
  macOS `RunEvent::Opened` handling, first-launch file-association
  advisory dialog.
- **Performance** — Criterion baselines for `compose()`, per-command
  `DirtyRegion`, Canvas2D sub-rect blit driven by a union-bounding-box
  dirty aggregator.
