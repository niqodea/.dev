vim.api.nvim_create_user_command('GitActiveFiles', function() vim.cmd('terminal git active-files') end, {})

vim.api.nvim_create_user_command('GitRecentFiles', function() vim.cmd('terminal git recent-files') end, {})

vim.api.nvim_create_user_command('GitStatus', function() vim.cmd('terminal git status --short') end, {})

local function run_git(arguments)
    local output = vim.fn.systemlist(vim.list_extend({ 'git' }, arguments))
    if vim.v.shell_error ~= 0 then
        error('git ' .. table.concat(arguments, ' ') .. ' failed: ' .. table.concat(output, ' '))
    end
    return output
end

local function complete_branch(argument)
    local branches = run_git({ 'for-each-ref', '--format=%(refname:short)', 'refs/heads', 'refs/remotes' })
    return vim.tbl_filter(function(branch) return vim.startswith(branch, argument) end, branches)
end

-- Lists, one quickfix entry per hunk, what git commit --all would put on the branch since it left its base
-- That leaves out untracked files, since git commit --all does not add them
local function diff_all()
    local base_branch = run_git({ 'base-branch' })[1]
    local branch_point = run_git({ 'merge-base', base_branch, 'HEAD' })[1]
    local root = run_git({ 'rev-parse', '--show-toplevel' })[1]
    -- --no-renames lists a renamed file as deleted and added, so each path is new or exists at the branch point
    -- core.quotePath=false keeps non-ASCII file names unquoted, as the header match reads them
    local diff_lines = run_git({ '-c', 'core.quotePath=false', 'diff', '--unified=0', '--no-renames', branch_point })
    local items = {}
    local path
    -- A changed line can look like a header line, so file names are read only from the header
    local in_header = false
    for _, line in ipairs(diff_lines) do
        if vim.startswith(line, 'diff --git ') then
            in_header = true
        elseif vim.startswith(line, '@@ ') then
            in_header = false
            local line_number = line:match('^@@ %-%S+ %+(%d+)')
            -- A hunk that only deletes the first lines starts at line 0
            table.insert(
                items,
                { filename = root .. '/' .. path, lnum = math.max(tonumber(line_number), 1), text = '' }
            )
        elseif in_header then
            -- A deleted file has only its old path, and an added one only its new path
            -- git ends a name that contains a space with a tab, which is not part of the name
            path = line:match('^%-%-%- a/(.-)\t?$') or line:match('^%+%+%+ b/(.-)\t?$') or path
        else
            local item = items[#items]
            -- An added line shows what the branch wrote, so it wins over a removed one
            if item.text == '' or (vim.startswith(line, '+') and not vim.startswith(item.text, '+')) then
                item.text = line
            end
        end
    end
    vim.fn.setqflist({}, ' ', { title = 'Changes since HEAD left ' .. base_branch, items = items })
    vim.cmd('copen')
end

-- The file in a buffer as it was at a commit, read-only and named after the commit
local function get_version_buf(commit, file_buf)
    local path = vim.api.nvim_buf_get_name(file_buf)
    local short_commit = run_git({ 'rev-parse', '--short', commit })[1]
    local buf = vim.fn.bufadd(short_commit .. ':' .. vim.fn.fnamemodify(path, ':.'))
    -- Another tab may have filled this buffer already, and a commit's version never changes
    if vim.b[buf].git_version_filled then return buf end
    -- Found from the path alone, since a file deleted with its folder leaves no folder to run git in
    local root = assert(vim.fs.root(path, '.git'), path .. ' is in no git repository')
    local relative_path = vim.fs.relpath(root, path)
    local listed = run_git({ '-C', root, 'ls-tree', '--name-only', commit, '--', relative_path })
    vim.bo[buf].buftype = 'nofile'
    vim.bo[buf].bufhidden = 'wipe'
    vim.fn.bufload(buf)
    -- A file added since that commit has no version there
    if #listed > 0 then
        local lines = run_git({ '-C', root, 'show', commit .. ':' .. relative_path })
        vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    end
    vim.bo[buf].modifiable = false
    vim.bo[buf].filetype = vim.bo[file_buf].filetype
    vim.b[buf].git_version_filled = true
    return buf
end

-- A tab holds one comparison, since Vim diffs all of a tab's diff windows together
local tab_taken_warning = 'This tab already shows a comparison, which <leader>g/ closes'

-- The windows of the current tab that show a version of a file to compare with it
local function get_panes()
    return vim.tbl_filter(
        function(window) return vim.w[window].git_file_window ~= nil end,
        vim.api.nvim_tabpage_list_wins(0)
    )
end

-- Shows a buffer beside a file's window, diffed against the file
local function open_pane(buf, split, file_window)
    local window = vim.api.nvim_open_win(buf, false, { split = split, win = file_window })
    vim.api.nvim_win_call(window, function() vim.cmd('diffthis') end)
    vim.w[window].git_file_window = file_window
    -- The file window can show another buffer by the time the pane closes
    vim.w[window].git_file_buf = vim.api.nvim_win_get_buf(file_window)
end

-- Opens the file as it was where HEAD left its base branch in a split to the right
local function open_diff()
    if #get_panes() > 0 then
        vim.notify(tab_taken_warning, vim.log.levels.WARN)
        return
    end
    local file_buf = vim.api.nvim_get_current_buf()
    local base_branch = run_git({ 'base-branch' })[1]
    local branch_point = run_git({ 'merge-base', base_branch, 'HEAD' })[1]
    local version_buf = get_version_buf(branch_point, file_buf)
    vim.cmd('diffthis')
    open_pane(version_buf, 'right', vim.api.nvim_get_current_win())
    -- Vim lines up the windows of a diff only as one scrolls, which a new window has not done yet
    vim.cmd('syncbind')
end

-- One side of a conflicted file, read-only and named after the side, as in ours:<file>
local function get_side_buf(side, lines, file_buf)
    local buf = vim.fn.bufadd(side .. ':' .. vim.fn.fnamemodify(vim.api.nvim_buf_get_name(file_buf), ':.'))
    vim.bo[buf].buftype = 'nofile'
    vim.bo[buf].bufhidden = 'wipe'
    vim.fn.bufload(buf)
    -- Another window can still show this buffer, read-only, from an earlier conflict on the same file
    vim.bo[buf].modifiable = true
    vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
    vim.bo[buf].modifiable = false
    vim.bo[buf].filetype = vim.bo[file_buf].filetype
    return buf
end

-- Opens ours to the left of a conflicted file and theirs to the right
local function open_conflict()
    if #get_panes() > 0 then
        vim.notify(tab_taken_warning, vim.log.levels.WARN)
        return
    end
    local file_buf = vim.api.nvim_get_current_buf()
    local path = vim.api.nvim_buf_get_name(file_buf)
    local root = assert(vim.fs.root(path, '.git'), path .. ' is in no git repository')
    -- One line per stage the file has in the index, as in "100644 <object> 2\t<path>"
    local unmerged = run_git({ '-C', root, 'ls-files', '--unmerged', '--', vim.fs.relpath(root, path) })
    if #unmerged == 0 then error(path .. ' has no conflict') end
    local objects_by_stage = {}
    for _, line in ipairs(unmerged) do
        local object, stage = line:match('^%S+ (%S+) (%d)\t')
        objects_by_stage[tonumber(stage)] = object
    end
    -- git keeps ours as stage 2 and theirs as stage 3, and a side that deleted the file has no stage
    local our_lines = objects_by_stage[2] and run_git({ '-C', root, 'cat-file', 'blob', objects_by_stage[2] }) or {}
    local their_lines = objects_by_stage[3] and run_git({ '-C', root, 'cat-file', 'blob', objects_by_stage[3] }) or {}
    local our_buf = get_side_buf('ours', our_lines, file_buf)
    local their_buf = get_side_buf('theirs', their_lines, file_buf)
    local file_window = vim.api.nvim_get_current_win()
    vim.cmd('diffthis')
    open_pane(our_buf, 'left', file_window)
    open_pane(their_buf, 'right', file_window)
end

-- Closes the current tab's comparison, and leaves diff mode in the compared file's window
local function close_comparison()
    local panes = get_panes()
    if #panes == 0 then return end
    local file_window = vim.w[panes[1]].git_file_window
    local file_buf = vim.w[panes[1]].git_file_buf
    for _, window in ipairs(panes) do
        vim.api.nvim_win_close(window, false)
    end
    -- Vim restores a file's last 'diff' in a window, so the file is shown there briefly for diffoff
    if vim.api.nvim_win_is_valid(file_window) and vim.api.nvim_buf_is_valid(file_buf) then
        vim.api.nvim_win_call(file_window, function()
            local view = vim.fn.winsaveview()
            local shown_buf = vim.api.nvim_get_current_buf()
            vim.cmd('noautocmd keepalt keepjumps buffer ' .. file_buf)
            vim.cmd('diffoff')
            vim.cmd('noautocmd keepalt keepjumps buffer ' .. shown_buf)
            vim.fn.winrestview(view)
        end)
    end
end

vim.keymap.set('n', '<leader>gd', open_diff)
vim.keymap.set('n', '<leader>gD', diff_all)
vim.keymap.set('n', '<leader>gm', open_conflict)
vim.keymap.set('n', '<leader>g/', close_comparison)
vim.api.nvim_create_user_command(
    'GitSetBase',
    function(args) run_git({ 'base-branch', args.args }) end,
    { nargs = 1, complete = complete_branch }
)
