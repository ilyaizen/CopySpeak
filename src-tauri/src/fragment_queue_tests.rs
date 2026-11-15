use super::*;

fn create_test_fragment(text: &str, index: usize, total: usize) -> TextFragment {
    TextFragment::new(text.to_string(), index, total)
}

#[test]
fn test_empty_queue() {
    let queue = FragmentQueue::new();
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    assert_eq!(queue.status(), QueueStatus::Idle);
}

#[test]
fn test_add_single_fragment() {
    let queue = FragmentQueue::new();
    let fragment = create_test_fragment("Hello world", 0, 1);
    queue.add_fragment(fragment);

    assert!(!queue.is_empty());
    assert_eq!(queue.len(), 1);
}

#[test]
fn test_add_multiple_fragments() {
    let queue = FragmentQueue::new();
    let fragments = vec![
        create_test_fragment("First", 0, 3),
        create_test_fragment("Second", 1, 3),
        create_test_fragment("Third", 2, 3),
    ];
    queue.add_fragments(fragments);

    assert_eq!(queue.len(), 3);
}

#[test]
fn test_clear_queue() {
    let queue = FragmentQueue::new();
    queue.add_fragment(create_test_fragment("Test", 0, 1));
    assert_eq!(queue.len(), 1);

    queue.clear();
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
}

#[test]
fn test_navigation_first_time() {
    let queue = FragmentQueue::new();
    queue.add_fragment(create_test_fragment("First", 0, 2));
    queue.add_fragment(create_test_fragment("Second", 1, 2));

    // Not playing yet, should return index 0
    assert_eq!(queue.next(), Some(0));

    // Move to index 0
    queue.skip_to(0).unwrap();
    assert_eq!(queue.current_index(), Some(0));

    // Next should return index 1
    assert_eq!(queue.next(), Some(1));

    // Move to index 1
    queue.skip_to(1).unwrap();
    assert_eq!(queue.current_index(), Some(1));

    // Next should return None (end of queue)
    assert_eq!(queue.next(), None);
}

#[test]
fn test_navigation_after_set_index() {
    let queue = FragmentQueue::new();
    queue.add_fragments(vec![
        create_test_fragment("First", 0, 3),
        create_test_fragment("Second", 1, 3),
        create_test_fragment("Third", 2, 3),
    ]);

    // Set current index to 1
    queue.skip_to(1).unwrap();
    assert_eq!(queue.current_index(), Some(1));

    // Next should be 2
    assert_eq!(queue.next(), Some(2));

    // Move to index 2
    queue.skip_to(2).unwrap();

    // Next should be None (at end)
    assert_eq!(queue.next(), None);
}

#[test]
fn test_navigation_previous() {
    let queue = FragmentQueue::new();
    queue.add_fragments(vec![
        create_test_fragment("First", 0, 3),
        create_test_fragment("Second", 1, 3),
        create_test_fragment("Third", 2, 3),
    ]);

    queue.skip_to(2).unwrap();
    assert_eq!(queue.current_index(), Some(2));

    assert_eq!(queue.previous(), Some(1));

    queue.skip_to(1).unwrap();
    assert_eq!(queue.current_index(), Some(1));

    assert_eq!(queue.previous(), Some(0));

    queue.skip_to(0).unwrap();
    assert_eq!(queue.current_index(), Some(0));

    assert_eq!(queue.previous(), None);
}

#[test]
fn test_skip_to() {
    let queue = FragmentQueue::new();
    queue.add_fragments(vec![
        create_test_fragment("First", 0, 3),
        create_test_fragment("Second", 1, 3),
        create_test_fragment("Third", 2, 3),
    ]);

    // Valid skip
    assert!(queue.skip_to(1).is_ok());
    assert_eq!(queue.current_index(), Some(1));

    // Invalid skip
    assert!(queue.skip_to(10).is_err());
}

#[test]
fn test_audio_storage() {
    let queue = FragmentQueue::new();
    queue.add_fragment(create_test_fragment("Test", 0, 1));

    assert!(!queue.has_audio(0));

    let fake_audio = vec![1, 2, 3, 4, 5];
    queue.set_audio(0, fake_audio.clone());

    assert!(queue.has_audio(0));
    assert_eq!(queue.get_audio(0), Some(fake_audio));
}

#[test]
fn test_status_transitions() {
    let queue = FragmentQueue::new();
    queue.add_fragment(create_test_fragment("Test", 0, 1));

    assert_eq!(queue.status(), QueueStatus::Idle);

    queue.start();
    assert_eq!(queue.status(), QueueStatus::Playing);

    queue.pause();
    assert_eq!(queue.status(), QueueStatus::Paused);

    queue.resume();
    assert_eq!(queue.status(), QueueStatus::Playing);

    queue.stop();
    assert_eq!(queue.status(), QueueStatus::Stopped);
}

#[test]
fn test_get_fragment() {
    let queue = FragmentQueue::new();
    let fragments = vec![
        create_test_fragment("First", 0, 2),
        create_test_fragment("Second", 1, 2),
    ];
    queue.add_fragments(fragments);

    let first = queue.get_fragment(0).unwrap();
    assert_eq!(first.text, "First");
    assert_eq!(first.index, 0);

    let second = queue.get_fragment(1).unwrap();
    assert_eq!(second.text, "Second");
    assert_eq!(second.index, 1);
}

#[test]
fn test_stop_flag() {
    let queue = FragmentQueue::new();

    assert!(!queue.should_stop());

    queue.stop();
    assert!(queue.should_stop());

    queue.clear_stop_flag();
    assert!(!queue.should_stop());
}

#[test]
fn test_clear_resets_stop_flag_and_status() {
    let queue = FragmentQueue::new();
    queue.add_fragment(create_test_fragment("Test", 0, 1));

    queue.stop();
    assert!(queue.should_stop());
    assert_eq!(queue.status(), QueueStatus::Stopped);

    // speak_queued clears before refilling; a flag left set here would
    // silently no-op every later queued reading.
    queue.clear();
    assert!(!queue.should_stop());
    assert_eq!(queue.status(), QueueStatus::Idle);
}
