-- Use space as leader
-- NOTE: Modules downstream depend on vim.g.mapleader being set!
vim.g.mapleader = '<Space>'
vim.keymap.set('', '<Space>', '')

require('own.core')

-- Extract module names from files
local module_files = vim.api.nvim_get_runtime_file('lua/own/modules/*.lua', true)
local modules = {}
for _, module_file in ipairs(module_files) do
    local module = module_file:match(".+/([^/]+)%.lua")
    table.insert(modules, module)
end

-- Create start commands for modules
for _, module in ipairs(modules) do
    local capitalized_module = module:gsub('^%l', string.upper)
    local start_command = 'OwnStart' .. capitalized_module
    local module_path = 'own.modules.' .. module
    vim.api.nvim_create_user_command(start_command, function()
        require(module_path)
    end, {})
end

-- Create start command for all modules
vim.api.nvim_create_user_command('OwnAssemble', function()
    for _, module in ipairs(modules) do
        local module_path = 'own.modules.' .. module
        require(module_path)
    end
end, {})

-- Create startup commands for automatic loading of modules
local atdir = require('own.core.utils').get_atdir()
local startup_path = atdir .. '/startup.nvim-own'
vim.api.nvim_create_user_command('OwnStartupEdit', function()
    vim.cmd('edit ' .. startup_path)
    if vim.fn.filereadable(startup_path) == 0 then
        local lines = {}
        for _, module in ipairs(modules) do
            table.insert(lines, '# ' .. module)
        end
        vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
    end
end, {})
vim.api.nvim_create_user_command('OwnStartupAssemble', function()
    local startup_file = io.open(startup_path, 'w')
    for _, module in ipairs(modules) do
        startup_file:write(module .. '\n')
    end
    startup_file:close()
end, {})
vim.api.nvim_create_user_command('OwnStartupRun', function()
    if vim.fn.filereadable(startup_path) == 0 then
        error('Startup file does not exist: ' .. startup_path)
    end
    local startup_file = io.open(startup_path, 'r')
    for line in startup_file:lines() do
        if line:match('^%s*#') or line:match('^%s*$') then
            goto continue
        end
        local module = line:match('^%s*(%S+)')
        local module_path = 'own.modules.' .. module
        require(module_path)
        ::continue::
    end
end, {})

-- Enable comfy leader + ctrl commands
vim.keymap.set('', '<C-Space>', '<Space>', {remap = true})
-- No leader timeout
vim.o.timeout = false

-- Easier split management
vim.keymap.set('n', vim.g.mapleader..'j', '<C-w>j')
vim.keymap.set('n', vim.g.mapleader..'k', '<C-w>k')
vim.keymap.set('n', vim.g.mapleader..'h', '<C-w>h')
vim.keymap.set('n', vim.g.mapleader..'l', '<C-w>l')
vim.keymap.set('n', vim.g.mapleader..'s', '<C-w>s')
vim.keymap.set('n', vim.g.mapleader..'v', '<C-w>v')
vim.keymap.set('n', vim.g.mapleader..'S', ':below split<cr>')
vim.keymap.set('n', vim.g.mapleader..'V', ':below vsplit<cr>')
-- Easier resizing of splits
vim.keymap.set('n', '<C-left>', '2<C-w><')
vim.keymap.set('n', '<C-down>', '2<C-w>+')
vim.keymap.set('n', '<C-up>', '2<C-w>-')
vim.keymap.set('n', '<C-right>', '2<C-w>>')

-- Easier tab handling
vim.keymap.set('n', vim.g.mapleader..'<C-t>', ':tabnew<cr>')
vim.keymap.set('n', vim.g.mapleader..'<C-n>', ':tabnext<cr>')
vim.keymap.set('n', vim.g.mapleader..'<C-p>', ':tabprevious<cr>')

-- Quick common actions
vim.keymap.set('n', vim.g.mapleader..'q', ':quit<cr>')
vim.keymap.set('n', vim.g.mapleader..'Q', ':quitall<cr>')
vim.keymap.set('n', vim.g.mapleader..'w', ':write<cr>')
vim.keymap.set('n', vim.g.mapleader..'W', ':wall<cr>')
vim.keymap.set('n', vim.g.mapleader..'e', ':edit<cr>')

-- Reload all buffers (basically the missing :eall)
vim.api.nvim_create_user_command('EditAll', require('utils').reload_buffers, {})
vim.keymap.set('n', vim.g.mapleader..'E', ':EditAll<cr>')

-- Quick yank to and put from plus register (system clipboard)
vim.keymap.set({'n', 'v'}, vim.g.mapleader..'y', '"+y')
vim.keymap.set({'n', 'v'}, vim.g.mapleader..'Y', '"+y$')  -- Y is yy without recursive mappings
vim.keymap.set({'n', 'v'}, vim.g.mapleader..'p', '"+p')
vim.keymap.set({'n', 'v'}, vim.g.mapleader..'P', '"+P')
vim.api.nvim_create_user_command('YankPath', require('utils').yank_path, {})
vim.keymap.set('n', vim.g.mapleader..'<C-y>', ':YankPath<cr>')

-- Remove instructions banner from netrw
vim.g.netrw_banner = 0
-- Quick file explorer (we use '-' for consistency with netrw)
vim.api.nvim_create_user_command('ExploreDirectory', require('utils').explore_directory, {})
vim.api.nvim_create_user_command('ExploreCwd', require('utils').explore_cwd, {})
vim.api.nvim_create_user_command('ExploreGitRoot', require('utils').explore_git_root, {})
vim.keymap.set('n', vim.g.mapleader..'-', ':ExploreDirectory<cr>')
vim.keymap.set('n', vim.g.mapleader..'__', ':ExploreCwd<cr>')
vim.keymap.set('n', vim.g.mapleader..'_g', ':ExploreGitRoot<cr>')
vim.keymap.set('n', vim.g.mapleader..'_v', ':Explore $VIRTUAL_ENV<cr>')
vim.keymap.set('n', vim.g.mapleader..'_~', ':Explore $HOME<cr>')
vim.keymap.set('n', vim.g.mapleader..'_/', ':Explore /<cr>')

-- Quick exit from terminal mode
vim.keymap.set('t', '<C-\\><C-\\>', '<C-\\><C-n>')

-- Clean highlighted text
vim.keymap.set('', vim.g.mapleader..'/', ':nohlsearch<cr>')

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

-- Color scheme
vim.cmd('colorscheme retrobox')
