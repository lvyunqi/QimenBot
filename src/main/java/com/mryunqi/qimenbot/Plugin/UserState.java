package com.mryunqi.qimenbot.Plugin;

import com.mikuac.shiro.common.utils.MsgUtils;
import com.mikuac.shiro.core.Bot;
import com.mikuac.shiro.core.BotPlugin;
import com.mikuac.shiro.dto.event.message.WholeMessageEvent;
import com.mryunqi.qimenbot.Controller.Function;
import com.mryunqi.qimenbot.Controller.User;
import com.mryunqi.qimenbot.Template.UserStateTemplate;
import org.jetbrains.annotations.NotNull;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Component;
import com.alibaba.fastjson.JSONObject;


/**
 * 指令：状态
 * @PluginName: 状态
 * @author mryunqi
 * @since 2022-6-22
 * @version 1.0
 */

@Component
public class UserState extends BotPlugin {
    private final JdbcTemplate jct;

    public UserState(JdbcTemplate jct) {
        this.jct = jct;
    }

    @Override
    public int onWholeMessage(@NotNull Bot bot, @NotNull WholeMessageEvent event) {
        String msg = event.getMessage();
        String userId = String.valueOf(event.getUserId());
        User userDefault = new User(userId);
        if ("状态".equals(msg) & !userDefault.Is_UserExist(jct)) {
            bot.sendMsg(event, "您还没有穿越到斗罗大陆！\n<可用命令>\n开始穿越", false);
            return MESSAGE_IGNORE;
        }
        if (!"状态".equals(msg) & !userDefault.Is_UserExist(jct)) return MESSAGE_IGNORE;
        if ("状态".equals(msg) & !userDefault.Is_UserAwake(jct)){
            bot.sendMsg(event, "您还没有武魂觉醒！\n<可用命令>\n武魂觉醒", false);
            return MESSAGE_IGNORE;
        }
        if (!"状态".equals(msg) & !userDefault.Is_UserAwake(jct)) return MESSAGE_IGNORE;
        String Alias = userDefault.Get_UserAlias(jct,"状态");
        if (Alias == null) return MESSAGE_IGNORE;
        JSONObject alias = JSONObject.parseObject(Alias);
        if ("状态".equals(msg) | alias.containsKey(msg)) {
            User user = new User(userId);
            Function func = new Function();
            String userData = user.Get_UserData(jct);
            String userSkill = user.Get_UserSkill(jct);
            int skillNum = func.Get_SkillNum(userSkill);
            String LvName = user.Get_UserLevelName(skillNum);
            UserStateTemplate userStateTemplate = new UserStateTemplate();
            String message = userStateTemplate.UserShelfState(userId,userData,"黄金龙枪","永恒之铠",LvName
            ,"锻造师","凤舞九天","[红]战神","[创神星级]永恒风暴",1);
            //构建消息
            String sendMsg = MsgUtils.builder()
                    .text(message)
                    .build();
            bot.sendMsg(event, sendMsg, false);
        }
        return MESSAGE_IGNORE;
    }
}
