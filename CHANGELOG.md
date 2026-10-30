# Changelog

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
