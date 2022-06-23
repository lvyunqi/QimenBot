package com.mryunqi.qimenbot.Template;

import com.alibaba.fastjson.JSON;
import com.alibaba.fastjson.JSONObject;
import com.mryunqi.qimenbot.Controller.Function;
import com.mryunqi.qimenbot.Controller.User;

public class UserStateTemplate {

    public String UserShelfState(String QQ,String  userData, String attackWeapon, String defendWeapon,String LvName
            ,String Job,String BattlePlate,String Mecha,String Warship,int UpExp) {
        String rootPath = System.getProperty("user.dir");
        Function func = new Function();
        User user = new User(QQ);
        JSONObject map = JSON.parseObject(userData);
        int UserSpirit = user.Get_UserSpirit(Integer.parseInt(map.getJSONObject("userData").getString("精神力")));
        String UserSpiritName = user.Get_UserSpiritName(Integer.parseInt(map.getJSONObject("userData").getString("精神力")));
        int NextLevelExp = func.Get_NextLevelExp(Integer.parseInt(map.getJSONObject("userData").getString("等级")),UpExp);
        double NextLevelExpPercent = func.Get_NextLevelExpPercent(Integer.parseInt(map.getJSONObject("userData").getString("经验")),NextLevelExp);
        int Ce = func.Get_Ce(userData);
        String t1 = "To" + map.getJSONObject("userInfo").getString("name") +"〔"+
                map.getJSONObject("userData").getString("武魂") +"〕\n";
        String t2 = "【"+ map.getJSONObject("userData").getString("武魂类型") +"】\n";
        String t3 = "[CQ:image,file=file:///"+rootPath+"/斗罗大陆图片/"+
                map.getJSONObject("userData").getString("武魂")+".jpg]\n";
        if (!attackWeapon.equals("")) {
            String t4 = "[攻击魂导器："+attackWeapon+"]\n";
            t3 += t4;
        }
        if (!defendWeapon.equals("")) {
            String t5 = "[防御魂导器："+defendWeapon+"]\n";
            t3 = t3 + t5;
        }
        if (!Job.equals("")) {
            String t6 = "职业："+Job+"\n";
            t3 = t3 + t6;
        }
        if (!BattlePlate.equals("")) {
            String t7 = "斗铠："+BattlePlate+"\n";
            t3 = t3 + t7;
        }
        if (!Mecha.equals("")) {
            String t8 = "机甲："+Mecha+"\n";
            t3 = t3 + t8;
        }
        if (!Warship.equals("")) {
            String t9 = "战舰："+Warship+"\n";
            t3 = t3 + t9;
        }
        String t10 = "·等级："+ map.getJSONObject("userData").getString("等级")+"["+LvName+"]\n";
        String t11 = "·经验："+ map.getJSONObject("userData").getString("经验") +"/" + NextLevelExp + "[" + String.format("%.2f", NextLevelExpPercent) +"%]\n";
        String t12 = "·战力："+ Ce +"\n";
        String t13 = "·精神力："+ map.getJSONObject("userData").getString("精神力") +"/" + UserSpirit + "["+ UserSpiritName +"]\n";
        String t14 = "·生命："+ map.getJSONObject("userData").getString("当前血量") + "/"
                + map.getJSONObject("userData").getString("血量") + "\n";
        String t15 = "·魂力："+ map.getJSONObject("userData").getString("当前魂力值") + "/"
                + map.getJSONObject("userData").getString("魂力值") + "\n";
        String t16 = "·攻击："+ map.getJSONObject("userData").getString("攻击") + "\n";
        String t17 = "·力量："+ map.getJSONObject("userData").getString("力量") + "\n";
        String t18 = "·防御："+ map.getJSONObject("userData").getString("防御") + "\n";
        String t19 = "·暴击："+ map.getJSONObject("userData").getString("暴击率") + "\n";
        String t20 = "·暴伤："+ map.getJSONObject("userData").getString("暴击伤害") + "\n";
        String t21 = "·速度："+ map.getJSONObject("userData").getString("速度") + "\n";
        String t22 = "·闪避："+ map.getJSONObject("userData").getString("闪避") + "\n";
        String t23 = "·体力值："+ map.getJSONObject("userData").getString("体力") + "\n";
        String t24 = "<可用命令>\n";
        String t25 = "背包\n";
        return t1+t2+t3+t10+t11+t12+t13+t14+t15+t16+t17+t18+t19+t20+t21+t22+t23+t24+t25;
    }
}
