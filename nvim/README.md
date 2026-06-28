# Neovim

Module for [Neovim](https://neovim.io/), a text editor.

## Concepts

### Leader keybinds

We use `leader` as a prefix for the majority of custom keybinds, which avoids messing with existing ones and makes it obvious which shortcuts are our own.
We map prefixed keybinds to actions that are similar to their non-prefixed counterparts, ensuring intuitive usage.

In environments with default Vim settings, we can simply ignore the original non-prefixed shortcuts.
This strategy is straightforward and requires minimal cognitive load.

We remap the `leader` key to <kbd>Space</kbd>, as it is easier to use as a prefix compared to <kbd>\</kbd>.
Another advantage of <kbd>Space</kbd> is its invariance with respect to <kbd>shift</kbd>, making it easy to use as prefix for both lowercase and uppercase keybinds.
In fact, we extend this property by also remapping <kbd>ctrl</kbd> + <kbd>Space</kbd> to simply <kbd>Space</kbd>, so that custom keybinds containing <kbd>ctrl</kbd> are also easy to type.

Since the `leader` key becomes quite central to our workflow, we also disable its timeout for ease of mind.

### Own mode

By running the `OwnStart` command we configure our current Neovim to operate in developer mode and provide additional functionalities.
This lets us have access to advanced features like LSP without bloating the experience of the vanilla text editor.
Upon entering developer mode, the runtime is augmented with a set of core functionalities.
If needed, additional optional lua modules can be imported with commands of the form `OwnModuleStart`.

Since running Neovim in developer mode might be a recurrent need in our workflow, we define an alias `vd` to immediately launch Neovim in developer mode.
Moreover, we provide a mechanism to configure workspace-specific startup lua modules with the `OwnCreateStartup` / `OwnRunStartup` command and define an alias `vds` to automatically import them at startup.
