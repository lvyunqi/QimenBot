package com.mryunqi.qimenbot;

import com.gitee.starblues.loader.launcher.SpringBootstrap;
import com.gitee.starblues.loader.launcher.SpringMainBootstrap;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

@SpringBootApplication
public class QimenBotApplication implements SpringBootstrap {

    public static void main(String[] args) {
        SpringMainBootstrap.launch(QimenBotApplication.class, args);
    }

    @Override
    public void run(String[] args) {
        // 在该实现方法中, 和 SpringBoot 使用方式一致
        SpringApplication.run(QimenBotApplication.class, args);
    }

}
