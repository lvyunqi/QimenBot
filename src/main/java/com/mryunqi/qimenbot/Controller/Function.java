package com.mryunqi.qimenbot.Controller;
import com.alibaba.fastjson.JSONObject;

public class Function {

    public int Get_SkillNum(String SkillData){
        if (SkillData == null) return 0;
        JSONObject obj = JSONObject.parseObject(SkillData);
        return obj.keySet().size();
    }

    /* 计算升级到下一等级需要的经验 */
    public int Get_NextLevelExp(int lv,int up){
        return ((lv * 1500) / 8) * up;
    }

    /* 距离下一等级经验百分比 */
    public double Get_NextLevelExpPercent(int exp, int up_exp){
        if (exp == 0){
            return 0.00;
        } else {
            return ((double) exp / (double) up_exp) * 100;
        }
    }

    /* 计算玩家战力 */
    public int Get_Ce(String UserData){
        JSONObject obj = JSONObject.parseObject(UserData);
        int HP = Integer.parseInt(obj.getJSONObject("userData").getString("血量"));
        int PR = Integer.parseInt(obj.getJSONObject("userData").getString("攻击"));
        int DE = Integer.parseInt(obj.getJSONObject("userData").getString("防御"));
        int SPEED = Integer.parseInt(obj.getJSONObject("userData").getString("速度"));
        int SP = Integer.parseInt(obj.getJSONObject("userData").getString("精神力"));
        return (int) ((0.12 * HP) + (0.75 * PR) + DE + (0.12 * SPEED) + (0.1 * SP) + 11);
    }
}
