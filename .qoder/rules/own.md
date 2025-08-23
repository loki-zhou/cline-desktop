---
trigger: always_on
alwaysApply: true
---
- 我的工作环境的是windows 的git-bash
- 比如 git diff 使用非分页模式: git --no-pager diff 
- 可以优先使用 ast-grep, ripgrep , sd, sed 等 shell 工具完成文本,代码查找,文本,代码替换等任务
- netstat -ano | findstr 25464 和 taskkill /F /PID 21368 这种命令需要在windows的powershell中完成,  除了这2个命令 之外, 不要再powershell使用其它命令. 其它命令必须得在git-bash中完成
- 有些知识点不清楚,可以搜索互联网去确定细节
