-- NOTE: Leader-prefixed keybind definitions depend on the custom leader key being set!
-- NOTE: Do _not_ use <Space>, <leader> substitution will consider the string literally!
leader = ' '  -- Space key. 
vim.g.mapleader = leader
vim.keymap.set('', leader, '')  -- Disable default leader key behavior in all modes.
vim.keymap.set('', '<C-' .. leader .. '>', leader, {remap = true})  -- Comfy leader + ctrl commands.
vim.o.timeout = false  -- No leader timeout.

require('x.core')

commands = require('x.commands')
utils = require('x.utils')

-- Extract module names from files
local modules_path = 'x.modules'
local modules_dir = 'lua/' .. modules_path:gsub('%.', '/')
local module_files = vim.api.nvim_get_runtime_file(modules_dir .. '/*.lua', true)
local modules = {}
for _, module_file in ipairs(module_files) do
    local module = module_file:match(modules_dir .. '/(.+)%.lua')
    table.insert(modules, module)
end

-- Create load commands for modules
for _, module in ipairs(modules) do
    local capitalized_module = module:gsub('^%l', string.upper)
    local load_command = 'XLoad' .. capitalized_module
    local module_path = modules_path .. '.' .. module
    vim.api.nvim_create_user_command(load_command, function()
        require(module_path)
    end, {})
end

-- Startup logic for automatic loading of modules
local atdir = utils.get_atdir()
local auto_path = atdir .. '/vx-autoload'

if vim.fn.filereadable(auto_path) == 1 then
    local auto_file = io.open(auto_path, 'r')
    for module in auto_file:lines() do
        local module_path = modules_path .. '.' .. module
        require(module_path)
        ::continue::
    end
    auto_file:close()
end

vim.cmd('colorscheme retrobox')

-- Easier split management
vim.keymap.set('n', '<leader>j', '<C-w>j')
vim.keymap.set('n', '<leader>k', '<C-w>k')
vim.keymap.set('n', '<leader>h', '<C-w>h')
vim.keymap.set('n', '<leader>l', '<C-w>l')
vim.keymap.set('n', '<leader>s', '<C-w>s')
vim.keymap.set('n', '<leader>v', '<C-w>v')
vim.keymap.set('n', '<leader>S', ':below split<cr>')
vim.keymap.set('n', '<leader>V', ':below vsplit<cr>')
-- Easier resizing of splits
vim.keymap.set('n', '<C-left>', '2<C-w><')
vim.keymap.set('n', '<C-down>', '2<C-w>+')
vim.keymap.set('n', '<C-up>', '2<C-w>-')
vim.keymap.set('n', '<C-right>', '2<C-w>>')

-- Easier tab handling
vim.keymap.set('n', '<leader><C-t>', ':tabnew<cr>')
vim.keymap.set('n', '<leader><C-n>', ':tabnext<cr>')
vim.keymap.set('n', '<leader><C-p>', ':tabprevious<cr>')

-- Quick common actions
vim.keymap.set('n', '<leader>q', ':quit<cr>')
vim.keymap.set('n', vim.g.mapleader .. 'Q', ':quitall<cr>')
vim.keymap.set('n', '<leader>w', ':write<cr>')
vim.keymap.set('n', '<leader>W', ':wall<cr>')
vim.keymap.set('n', '<leader>e', ':edit<cr>')

-- Reload all buffers (basically the missing :eall)
vim.api.nvim_create_user_command('EditAll', commands.reload_buffers, {})
vim.keymap.set('n', '<leader>E', ':EditAll<cr>')

-- Quick yank to and put from plus register (system clipboard)
vim.keymap.set({'n', 'v'}, '<leader>y', '"+y')
vim.keymap.set({'n', 'v'}, '<leader>Y', '"+y$')  -- Y is yy without recursive mappings
vim.keymap.set({'n', 'v'}, '<leader>p', '"+p')
vim.keymap.set({'n', 'v'}, '<leader>P', '"+P')
vim.api.nvim_create_user_command('YankPath', commands.yank_path, {})
vim.keymap.set('n', '<leader><C-y>', ':YankPath<cr>')

-- Remove instructions banner from netrw
vim.g.netrw_banner = 0
-- Quick file explorer (we use '-' for consistency with netrw)
vim.api.nvim_create_user_command('ExploreDirectory', commands.explore_directory, {})
vim.api.nvim_create_user_command('ExploreCwd', commands.explore_cwd, {})
vim.api.nvim_create_user_command('ExploreGitRoot', commands.explore_git_root, {})
vim.keymap.set('n', '<leader>-', ':ExploreDirectory<cr>')
vim.keymap.set('n', '<leader>__', ':ExploreCwd<cr>')
vim.keymap.set('n', '<leader>_g', ':ExploreGitRoot<cr>')
vim.keymap.set('n', '<leader>_v', ':Explore $VIRTUAL_ENV<cr>')
vim.keymap.set('n', '<leader>_~', ':Explore $HOME<cr>')
vim.keymap.set('n', '<leader>_/', ':Explore /<cr>')

-- Quick exit from terminal mode
vim.keymap.set('t', '<C-\\><C-\\>', '<C-\\><C-n>')

-- Clean highlighted text
vim.keymap.set('', '<leader>/', ':nohlsearch<cr>')

-- Turn tab into 4 spaces
vim.o.expandtab = true
vim.o.tabstop = 4
vim.o.shiftwidth = 4

-- Disable mouse features
vim.o.mouse = ""

-- Use smaller updatetime, used to make some things more responsive, such as git diff markers
vim.o.updatetime = 500

-- Show hybrid line numbers
-- Ref: https://jeffkreeftmeijer.com/vim-number
vim.o.number = true
vim.o.relativenumber = true
