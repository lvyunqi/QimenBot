package com.mryunqi.qimenbot.Plugin;

import com.mikuac.shiro.common.utils.MsgUtils;
import com.mikuac.shiro.core.Bot;
import com.mikuac.shiro.core.BotPlugin;
import com.mikuac.shiro.dto.event.message.WholeMessageEvent;
import org.jetbrains.annotations.NotNull;
import org.springframework.stereotype.Component;

@Component
public class Menu extends BotPlugin {
    @Override
    public int onWholeMessage(@NotNull Bot bot, @NotNull WholeMessageEvent event) {
        String msg = event.getMessage();
        if ("斗罗系统".equals(msg)){
            //构建消息
            String sendMsg = MsgUtils.builder()
                    .text("╭═══★斗罗大陆★═══╮\n角色菜单【☆】魂灵菜单\n魂骨菜单【☆】神位菜单\n职业菜单【☆】斗铠菜单\n副本菜单【☆】排行菜单\n交易菜单【☆】队伍菜单\n地图菜单【☆】任务菜单\n战斗菜单【☆】剧情菜单\n关系菜单【☆】万兽菜单\n╰═══★══════★═══╯")
                    .build();
            bot.sendMsg(event,sendMsg,false);
        }
        return MESSAGE_IGNORE;
    }
}
