-- A mapping reads <leader> when it is made, so the leader is set before any mapping
-- The leader is a literal space, since <leader> would take the text <Space> as is
local leader = ' '
vim.g.mapleader = leader
vim.keymap.set('', leader, '')  -- Space alone does nothing, instead of moving the cursor
vim.keymap.set('', '<C-' .. leader .. '>', leader, {remap = true})  -- So Ctrl can stay held through mappings like <leader><C-t>
vim.o.timeout = false  -- A mapping waits for its next key as long as it takes

require('x.core')

local commands = require('x.commands')
local utils = require('x.utils')

local modules_path = 'x.modules'
local modules_dir = 'lua/' .. modules_path:gsub('%.', '/')
local module_files = vim.api.nvim_get_runtime_file(modules_dir .. '/*.lua', true)
local modules = {}
for _, module_file in ipairs(module_files) do
    local module = module_file:match(modules_dir .. '/(.+)%.lua')
    table.insert(modules, module)
end

for _, module in ipairs(modules) do
    local capitalized_module = module:gsub('^%l', string.upper)
    local load_command = 'XLoad' .. capitalized_module
    local module_path = modules_path .. '.' .. module
    vim.api.nvim_create_user_command(load_command, function()
        require(module_path)
    end, {})
end

-- Set before loading modules, as it clears the highlights they set
vim.cmd('colorscheme retrobox')

local atdir = utils.get_atdir()
local auto_path = atdir .. '/vx-autoload'

if vim.fn.filereadable(auto_path) == 1 then
    local auto_file = io.open(auto_path, 'r')
    for module in auto_file:lines() do
        local module_path = modules_path .. '.' .. module
        require(module_path)
    end
    auto_file:close()
end

-- Pick the modules to load, now and automatically from then on
vim.api.nvim_create_user_command('XAutoload', function()
    require('x.core.fuzzy').fzf_lua.fzf_exec(modules, {
        fzf_opts = { ['--multi'] = true },
        actions = {
            ['default'] = function(selected)
                vim.fn.writefile(selected, auto_path)
                for _, module in ipairs(selected) do
                    require(modules_path .. '.' .. module)
                end
            end,
        },
    })
end, {})

vim.keymap.set('n', '<leader>j', '<C-w>j')
vim.keymap.set('n', '<leader>k', '<C-w>k')
vim.keymap.set('n', '<leader>h', '<C-w>h')
vim.keymap.set('n', '<leader>l', '<C-w>l')
vim.keymap.set('n', '<leader>s', '<C-w>s')
vim.keymap.set('n', '<leader>v', '<C-w>v')
vim.keymap.set('n', '<leader>S', ':below split<cr>')
vim.keymap.set('n', '<leader>V', ':below vsplit<cr>')
local function has_window(direction)
    return vim.fn.winnr(direction) ~= vim.fn.winnr()
end
-- An arrow moves a border in its direction: the right or bottom one when a window is there, otherwise the left or top one when a window is there
vim.keymap.set('n', '<C-left>', function() return has_window('l') and '2<C-w><' or has_window('h') and '2<C-w>>' or '' end, { expr = true })
vim.keymap.set('n', '<C-down>', function() return has_window('j') and '2<C-w>+' or has_window('k') and '2<C-w>-' or '' end, { expr = true })
vim.keymap.set('n', '<C-up>', function() return has_window('j') and '2<C-w>-' or has_window('k') and '2<C-w>+' or '' end, { expr = true })
vim.keymap.set('n', '<C-right>', function() return has_window('l') and '2<C-w>>' or has_window('h') and '2<C-w><' or '' end, { expr = true })
-- An arrow moves the window into its neighbor's cell, split across the direction of travel, so repeated moves reach any cell
vim.keymap.set('n', '<Left>', function() commands.move_window('h', 'above') end)
vim.keymap.set('n', '<Down>', function() commands.move_window('j', 'left') end)
vim.keymap.set('n', '<Up>', function() commands.move_window('k', 'right') end)
vim.keymap.set('n', '<Right>', function() commands.move_window('l', 'below') end)

vim.keymap.set('n', '<leader><C-t>', ':tabnew<cr>')
vim.keymap.set('n', '<leader><C-n>', ':tabnext<cr>')
vim.keymap.set('n', '<leader><C-p>', ':tabprevious<cr>')

vim.keymap.set('n', '<leader>q', ':quit<cr>')
vim.keymap.set('n', '<leader>Q', ':quitall<cr>')
vim.keymap.set('n', '<leader>w', ':write<cr>')
vim.keymap.set('n', '<leader>W', ':wall<cr>')
vim.keymap.set('n', '<leader>e', ':edit<cr>')

-- Reload every buffer, the :eall that vim lacks
vim.api.nvim_create_user_command('EditAll', commands.reload_buffers, {})
vim.keymap.set('n', '<leader>E', ':EditAll<cr>')

-- The + register is the system clipboard
vim.keymap.set({'n', 'v'}, '<leader>y', '"+y')
vim.keymap.set({'n', 'v'}, '<leader>Y', '"+y$')  -- Not "+Y, since nvim's Y-to-the-end is a mapping that this one does not follow
vim.keymap.set({'n', 'v'}, '<leader>p', '"+p')
vim.keymap.set({'n', 'v'}, '<leader>P', '"+P')
vim.api.nvim_create_user_command('YankPath', commands.yank_path, {})
vim.keymap.set('n', '<leader><C-y>', ':YankPath<cr>')

vim.keymap.set('t', '<C-\\><C-\\>', '<C-\\><C-n>')

vim.keymap.set('', '<leader>/', ':nohlsearch<cr>')

vim.o.expandtab = true
vim.o.tabstop = 4
vim.o.shiftwidth = 4

vim.o.mouse = ""

-- What waits for the cursor to rest, like git diff markers, updates after half a second instead of four
vim.o.updatetime = 500

-- Show hybrid line numbers
-- Ref: https://jeffkreeftmeijer.com/vim-number
vim.o.number = true
vim.o.relativenumber = true
