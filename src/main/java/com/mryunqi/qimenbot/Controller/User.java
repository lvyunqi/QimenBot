package com.mryunqi.qimenbot.Controller;

import org.springframework.jdbc.core.JdbcTemplate;
import com.alibaba.fastjson.JSON;
import com.alibaba.fastjson.JSONObject;

import java.util.List;


/**
 * @author mryunqi
 * @since 2022-6-22
 * @version 1.0
 */

public class User {
    String QQ;

    public User(String QQ) {
        this.QQ = QQ;
    }

    /* 获取玩家数据 */
    public String Get_UserData(JdbcTemplate jct) {
        String sql = "SELECT state_info FROM user WHERE qq=" + User.this.QQ;
        return jct.queryForObject(sql, String.class);
    }

    /* 获取特定alias */
    public String Get_UserAlias(JdbcTemplate jct, String alias) {
        String sql = "SELECT alias FROM user WHERE qq=" + User.this.QQ;
        String Alias = jct.queryForObject(sql, String.class);
        if (Alias == null) return null;
        JSONObject obj = JSON.parseObject(Alias);
        if (obj.containsKey(alias)) return obj.getString(alias);
        else return null;
    }

    /* 判断是否存在此玩家 */
    public boolean Is_UserExist(JdbcTemplate jct) {
        String sql = "SELECT qq FROM user";
        List<String> list = jct.queryForList(sql, String.class);
        return list.contains(String.valueOf(User.this.QQ));
    }

    /* 判断玩家是否觉醒武魂 */
    public boolean Is_UserAwake(JdbcTemplate jct) {
        String sql = "SELECT state_info FROM user WHERE qq=" + User.this.QQ;
        return jct.queryForObject(sql, String.class) != null;
    }

    /* 获取玩家等级称号*/
    public String Get_UserLevelName(int lv) {
        switch (lv) {
            case 0:
                return "魂士";
            case 1:
                return "一环魂师";
            case 2:
                return "二环大魂师";
            case 3:
                return "三环魂尊";
            case 4:
                return "三环魂宗";
            case 5:
                return "五环魂王";
            case 6:
                return "六环魂帝";
            case 7:
                return "七环魂圣";
            case 8:
                return "八环魂斗罗";
            case 9:
                return "九环封号斗罗";
            case 10:
                return "三级神祇";
            case 11:
                return "二级神祇";
            case 12:
                return "一级神祇";
            case 13:
                return "神界执法者";
            case 14:
                return "至高神";
            case 15:
                return "神王";
            default:
                return "？？？";
        }
    }

    /* 获取玩家Skill*/
    public String Get_UserSkill(JdbcTemplate jct) {
        String sql = "SELECT skill FROM user WHERE qq=" + User.this.QQ;
        return jct.queryForObject(sql, String.class);
    }

    /* 获取玩家下一阶精神力 */
    public int Get_UserSpirit(int SP) {
        if (SP <= 99) return 100;
        else if (SP <= 499) return 500;
        else if (SP <= 4999) return 5000;
        else if (SP <= 19999) return 20000;
        else if (SP <= 49999) return 50000;
        else return 9999999;
    }

    /* 获取玩家精神力等阶名称 */
    public String Get_UserSpiritName(int SP) {
        if (SP <= 99) return "灵元境";
        else if (SP <= 499) return "灵通境";
        else if (SP <= 4999) return "灵海境";
        else if (SP <= 19999) return "灵渊境";
        else if (SP <= 49999) return "灵域境";
        else return "神元境";
    }
}
