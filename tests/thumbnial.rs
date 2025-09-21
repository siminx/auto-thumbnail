#[cfg(test)]
mod tests {

    use auto_thumbnail::Thumbnailer;

    #[test]
    fn it_works() {
        let thumbnailer = Thumbnailer::default();

        let result = thumbnailer.create_thumbnail("demo/1.webp", "demo/output1.webp");
        assert!(result.is_ok());

        let result = thumbnailer.create_thumbnail("demo/2.png", "demo/output2.png");
        assert!(result.is_ok());

        let result = thumbnailer.create_thumbnail("demo/3.jpg", "demo/output3.jpg");
        assert!(result.is_ok());

        let result = thumbnailer.create_thumbnail("demo/4.pdf", "demo/output4.webp");
        assert!(result.is_ok());

        let result = thumbnailer.create_thumbnail("demo/5.mp4", "demo/output5.webp");
        assert!(result.is_ok());
    }
}
