vim.api.nvim_create_user_command('GitActiveFiles', function()
    vim.cmd('terminal git active-files')
end, {})

vim.api.nvim_create_user_command('GitRecentFiles', function()
    vim.cmd('terminal git recent-files')
end, {})

vim.api.nvim_create_user_command('GitStatus', function()
    vim.cmd('terminal git status --short')
end, {})

local function get_branch_point(branch)
    local output = vim.fn.systemlist({ 'git', 'merge-base', branch, 'HEAD' })
    if vim.v.shell_error ~= 0 then
        error('Cannot find where HEAD left ' .. branch .. ': ' .. table.concat(output, ' '))
    end
    return output[1]
end

-- A detached HEAD is on no branch, so there is no name
local function get_current_branch()
    return vim.fn.systemlist({ 'git', 'branch', '--show-current' })[1]
end

-- git does not record which branch a branch started from, so the branch keeps it in a setting of its own
local function get_base_branch_key(branch)
    return 'branch.' .. branch .. '.dotdev-base'
end

-- origin/HEAD stands in on a detached HEAD, and on a branch with no base of its own
local function get_base_branch()
    local current_branch = get_current_branch()
    if current_branch == nil then
        return 'origin/HEAD'
    end
    local output = vim.fn.systemlist({ 'git', 'config', '--get', get_base_branch_key(current_branch) })
    if vim.v.shell_error ~= 0 then
        return 'origin/HEAD'
    end
    return output[1]
end

local function complete_branch(argument)
    local branches = vim.fn.systemlist({ 'git', 'for-each-ref', '--format=%(refname:short)', 'refs/heads', 'refs/remotes' })
    return vim.tbl_filter(function(branch) return vim.startswith(branch, argument) end, branches)
end

-- Lists every change since HEAD left its base branch, committed or not, one quickfix entry per hunk
local function review()
    local base_branch = get_base_branch()
    local branch_point = get_branch_point(base_branch)
    local root = vim.fn.systemlist({ 'git', 'rev-parse', '--show-toplevel' })[1]
    local diff = vim.fn.systemlist({ 'git', 'diff', '--unified=0', branch_point })
    local items = {}
    local path
    for _, line in ipairs(diff) do
        if vim.startswith(line, '+++ ') then
            -- A deleted file has no b/ path, and no lines left to jump to
            path = line:match('^%+%+%+ b/(.*)')
        end
        local line_number = line:match('^@@ %-%S+ %+(%d+)')
        if line_number ~= nil and path ~= nil then
            -- A hunk that only deletes the first lines starts at line 0
            table.insert(items, { filename = root .. '/' .. path, lnum = math.max(tonumber(line_number), 1), text = line })
        end
    end
    vim.fn.setqflist({}, ' ', { title = 'Review against ' .. base_branch, items = items })
    vim.cmd('copen')
end

local function get_lines_at(commit, path)
    local folder = vim.fn.fnamemodify(path, ':h')
    local object = commit .. ':./' .. vim.fn.fnamemodify(path, ':t')
    vim.fn.system({ 'git', '-C', folder, 'cat-file', '-e', object })
    -- A file added since that commit has no earlier version
    if vim.v.shell_error ~= 0 then
        return {}
    end
    return vim.fn.systemlist({ 'git', '-C', folder, 'show', object })
end

-- Opens the file as it was where HEAD left its base branch, side by side with the file now
local function diff()
    local base_branch = get_base_branch()
    local branch_point = get_branch_point(base_branch)
    local lines = get_lines_at(branch_point, vim.fn.expand('%:p'))
    local filetype = vim.bo.filetype
    vim.cmd('diffthis')
    vim.cmd('leftabove vnew')
    vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
    vim.bo.buftype = 'nofile'
    vim.bo.bufhidden = 'wipe'
    vim.bo.modifiable = false
    vim.bo.filetype = filetype
    vim.cmd('diffthis')
    -- Closing the old version takes the file out of diff mode too
    vim.api.nvim_create_autocmd('BufWipeout', { buffer = 0, command = 'diffoff!' })
    vim.notify('Comparing with where HEAD left ' .. base_branch)
end

vim.keymap.set('n', '<leader>gr', review)
vim.keymap.set('n', '<leader>gd', diff)
vim.api.nvim_create_user_command('GitSetBase', function(args)
    local current_branch = get_current_branch()
    if current_branch == nil then
        error('HEAD is on no branch, so there is no base to set')
    end
    local output = vim.fn.system({ 'git', 'config', get_base_branch_key(current_branch), args.args })
    if vim.v.shell_error ~= 0 then
        error('Cannot set the base branch: ' .. output)
    end
end, { nargs = 1, complete = complete_branch })
