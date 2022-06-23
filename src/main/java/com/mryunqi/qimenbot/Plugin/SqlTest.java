package com.mryunqi.qimenbot.Plugin;

import com.mikuac.shiro.common.utils.MsgUtils;
import com.mikuac.shiro.core.Bot;
import com.mikuac.shiro.core.BotPlugin;
import com.mikuac.shiro.dto.event.message.WholeMessageEvent;
import org.jetbrains.annotations.NotNull;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Component;

import java.util.List;
import java.util.Map;

@Component
public class SqlTest extends BotPlugin {

    private final JdbcTemplate jct;

    public SqlTest(JdbcTemplate jct) {
        this.jct = jct;
    }

    @Override
    public int onWholeMessage(@NotNull Bot bot, @NotNull WholeMessageEvent event) {
        String msg = event.getMessage();
        if ("测试".equals(msg)){
            //查询数据库数据
            String sql = "SELECT name FROM user WHERE qq=434658198";
            List<Map<String, Object>> value = jct.queryForList(sql);
            for (Map<String, Object> map : value) {
                String name = (String) map.get("name");
                //构建消息
                String sendMsg = MsgUtils.builder()
                        .text("你的名字是："+name)
                        .build();
                bot.sendMsg(event,sendMsg,false);
            }
        }
        return MESSAGE_IGNORE;
    }

}
